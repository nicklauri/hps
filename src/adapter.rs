use crate::{
    config::{Matcher, CONFIG},
    utils::{self, RangeExt},
};
use anyhow::{anyhow, bail, Context, Error, Result};
use std::{collections::HashMap, net::SocketAddr, ops::Range};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::info;

#[derive(Debug)]
pub struct Adapter {
    client: TcpStream,
    client_addr: SocketAddr,
    buf_client: Vec<u8>,
    buf_server: Vec<u8>,

    // Amount of exceed data sent from client.
    // If request has "Content-Length", this is the amount of data for current request.
    // If request has no "Content-Length", this will be considered data for next request.
    remain_req_len: usize,

    // Cursor for client exceed data from self.buf_client.
    // If current request doesn't have a content-length,
    // copy [remain_req_start..remain_req_start+remain_req_len] to [0..remain_req_len] and start parsing.
    remain_req_start: usize,

    // Client's "Content-Length", will be reduced to zero to complete request.
    client_pending_read: usize,
}

impl Adapter {
    pub fn new(client: TcpStream, client_addr: SocketAddr) -> Self {
        Self {
            client,
            client_addr,
            buf_client: vec![0u8; CONFIG.buffer_size],
            buf_server: vec![0u8; CONFIG.buffer_size],
            remain_req_len: 0,
            remain_req_start: 0,
            client_pending_read: 0,
        }
    }

    pub async fn get_req_path<'a>(&'a mut self) -> Result<&'a str> {
        loop {
            let path_range = self.read_request_path_inner_str().await?;
            if let Some(range) = path_range {
                return self
                    .buf_client
                    .as_slice()
                    .get(range)
                    .with_context(|| {
                        format!(
                            "client={}: internal error: read_request_path_inner_str returned an invalid range",
                            self.client_addr
                        )
                    })
                    .and_then(|s| {
                        std::str::from_utf8(s)
                            .with_context(|| format!("client={}: request path is not valid UTF-8", self.client_addr))
                    });
            }
        }
    }

    async fn read_request_path_inner_str(&mut self) -> Result<Option<Range<usize>>> {
        let buf_cursor = self.remain_req_start;

        if buf_cursor >= self.buf_client.len() {
            // Use enum error and downcast to handle error more efficient.
            self.request_too_large().await;
            let _ = self.client.shutdown().await;
            bail!("request too large.");
        }

        let amount = self.client.read(&mut self.buf_client[buf_cursor..]).await?;
        if amount == 0 {
            bail!("bad request: read 0 bytes (remaining bytes={})", self.remain_req_len);
        }

        let total_buf_size = amount + buf_cursor;
        let buf = &self.buf_client[..total_buf_size];
        let parse_result = match utils::parse_request(buf)? {
            Some(result) => result,
            None => {
                self.remain_req_start = total_buf_size;
                self.remain_req_len = 0;
                return Ok(None); // back to the beginning of the loop.
            }
        };

        // Update for request's content-length or exceed data.
        self.remain_req_len = total_buf_size.saturating_sub(parse_result.parsed_len);
        self.remain_req_start = parse_result.parsed_len;
        self.client_pending_read = parse_result.content_length;

        parse_result
            .path
            .map(|p| self.buf_client.as_slice().range_from_part(p.as_bytes()))
            .context("bad request: no path found in request header")
            .map(Some)
    }

    pub async fn forward_request_to_server(&mut self, server: &mut TcpStream) -> Result<usize> {
        let mut bytes_read = 0usize;

        // Check if buf_client contains anything.
        if self.remain_req_start == 0 {
            return Ok(bytes_read);
        }

        // Request header.
        let buf = &self.buf_client[..self.remain_req_start];
        server.write_all(buf).await?;

        bytes_read += buf.len() + self.client_pending_read;

        // Move exceed data to the start of the buffer.
        self.buf_client
            .copy_within(self.remain_req_start..self.remain_req_start + self.remain_req_len, 0);

        self.remain_req_start = 0;

        // self.forward_client_content_length(server).await?;

        let result = utils::copy_nbuf(&mut self.client, server, &mut self.buf_client, self.client_pending_read).await;

        match result {
            Ok(()) => Ok(bytes_read),
            Err(err) => {
                if let Some(remaining) = err.downcast_ref::<usize>() {
                    Err(anyhow!("client reached EOF (remaining: {remaining} bytes)"))
                } else {
                    Err(err)
                }
            }
        }
    }

    pub async fn forward_response_to_client(&mut self, server: &mut TcpStream) -> Result<usize> {
        let mut buf_cur = 0usize;
        let pending_read;

        let parse_result = loop {
            let buf = &mut self.buf_server[buf_cur..];
            let amount = server.read(buf).await?;
            if amount == 0 {
                bail!("server stream reached EOF");
            }

            let total_buf_size = buf_cur + amount;
            let buf = &self.buf_server[..total_buf_size];
            let parse_result = match utils::parse_response(buf)? {
                Some(result) => result,
                None => {
                    buf_cur = total_buf_size;
                    continue;
                }
            };

            self.client.write_all(buf).await?;

            pending_read = parse_result.content_length - (total_buf_size - parse_result.parsed_len);

            break parse_result;
        };

        let bytes_read = pending_read + parse_result.parsed_len;

        let _ = utils::copy_nbuf(server, &mut self.client, &mut self.buf_server, pending_read).await?;

        Ok(bytes_read)
    }

    async fn run_inner(mut self) -> Result<()> {
        let mut servers = HashMap::<&Matcher, TcpStream>::new();
        let client_addr = self.client_addr;

        loop {
            let path = self.get_req_path().await?;

            let match_path_result = CONFIG
                .match_path(path)
                .with_context(|| format!("client={}: path {path:?} has no match", client_addr));

            let matcher = match match_path_result {
                Ok(matcher) => matcher,
                Err(err) => {
                    let _ = self.bad_request().await;
                    let _ = self.client.shutdown().await;
                    return Err(err);
                }
            };

            let server = match servers.get_mut(&matcher) {
                Some(server) => server,
                None => servers
                    .entry(&matcher)
                    .or_insert(matcher.create_connection().await.with_context(|| {
                        format!(
                            "client={}: connect to server={} failed",
                            self.client_addr,
                            matcher.server_addr()
                        )
                    })?),
            };

            if CONFIG.verbose {
                info!(
                    "client={}: connected to server={}",
                    self.client_addr,
                    matcher.server_addr()
                );
            }

            self.forward_request_to_server(server).await?;
            self.forward_response_to_client(server).await?;
        }
    }

    pub async fn run(self) -> Result<()> {
        self.run_inner().await
    }

    pub async fn request_too_large(&mut self) -> Error {
        self.send_client_error(
            REQUEST_TOO_LARGE,
            "Your browser send a very large request. Me server weak, sowwy :(",
        )
        .await
    }

    pub async fn bad_request(&mut self) -> Error {
        self.send_client_error(BAD_REQUEST, "Your browser sent a bad request.")
            .await
    }

    pub async fn send_client_error(&mut self, response_line: &str, response_content: &str) -> Error {
        let html = generate_html_content(response_line, response_content);
        let response = generate_http_html_response(response_line, &html);

        let response = response.as_bytes();

        let _ = self.client.write_all(response).await;
        let _ = self.client.shutdown().await;

        anyhow!("client={}: {}", self.client_addr, response_line)
    }
}

fn generate_http_html_response(status_text: &str, content: &str) -> String {
    format!(
        concat!(
            "HTTP/1.1 {}\r\n",
            "server: hps\r\n",
            "content-type: text/html\r\n",
            "content-length: {}\r\n",
            "connection: closed\r\n\r\n",
            "{}"
        ),
        status_text,
        content.len(),
        content
    )
}

fn generate_html_content(title: &str, body: &str) -> String {
    format!(
        concat!(
            "<!DOCTYPE>\
        <html>\
        <head>\
        <title>{}</title>\
        </head>\
        <body>\
        <h1>{}</h1>\
        <pre>{}</pre>\
        <pre>hps server version 1.0</pre>\
        </body>\
        </html>"
        ),
        title, title, body
    )
}

const BAD_REQUEST: &str = "400 Bad Request"; // 400
#[allow(dead_code)]
const URI_TOO_LONG: &str = "414 URI Too Long"; // 414
const REQUEST_TOO_LARGE: &str = "431 Request Entity Too Large"; // 431
