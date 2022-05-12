// get matched server from HpsConfig.
// create client from matched server address.
// establish bridge: client <-> hps <-> server

use crate::{
    config::{self, HpsConfig, CONFIG, MAX_NUMBERS_OF_HEADERS},
    server,
};
use anyhow::{anyhow, bail, Context, Error, Result};
use httparse::{Header, Request, Response, Status};
use std::{convert::Infallible, future::Future, mem::MaybeUninit, net::SocketAddr, sync::Arc};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    select,
};
use tracing::{info, warn};

fn get_path(src: &[u8]) -> Option<&str> {
    if src.contains(&b'\n') {
        let path = src.split(|&ch| ch == b' ').nth(1)?;
        std::str::from_utf8(path).ok()
    } else {
        None
    }
}

#[derive(Debug)]
pub struct Adapter {
    client: TcpStream,
    client_addr: SocketAddr,
    buf_client: Vec<u8>,
    buf_server: Vec<u8>,
    remain_req_len: usize,
    client_pending_read: usize,
    server_pending_read: usize,
}

impl Adapter {
    pub fn new(client: TcpStream, client_addr: SocketAddr) -> Self {
        Self {
            client,
            client_addr,
            buf_client: vec![0u8; CONFIG.buffer_size],
            buf_server: vec![0u8; CONFIG.buffer_size],
            remain_req_len: 0,
            client_pending_read: 0,
            server_pending_read: 0,
        }
    }

    pub async fn get_req_path(&mut self) -> Result<Option<&str>> {
        if self.remain_req_len >= self.buf_client.len() {
            return Err(self.request_too_large().await);
        }

        let amount = self
            .client
            .read(&mut self.buf_client[self.remain_req_len..])
            .await?;

        if amount == 0 {
            if CONFIG.verbose {
                warn!(
                    "client={}: bad request: read 0 bytes (remaining bytes={})",
                    self.client_addr, self.remain_req_len
                );
            }
            return Err(self.bad_request().await);
        }

        let total_buf_size = amount + self.remain_req_len;
        let buf = &self.buf_client[..total_buf_size];

        let mut headers: [MaybeUninit<Header<'_>>; MAX_NUMBERS_OF_HEADERS] =
            unsafe { MaybeUninit::uninit().assume_init() };

        let mut request = Request::new(&mut []);

        let parsed_len = match request.parse_with_uninit_headers(buf, &mut headers) {
            Ok(Status::Complete(parsed_len)) => parsed_len,
            Ok(Status::Partial) => return Ok(None), // Wait and rerun
            Err(err) => {
                if CONFIG.verbose {
                    warn!("client={}: parse header error.", self.client_addr);
                }

                self.bad_request().await;
                bail!(err)
            }
        };

        // parse and get content-length

        Ok(None)
    }

    pub async fn forward_request_to_server(&mut self) {}

    pub async fn run(mut self) -> Result<()> {
        Ok(())
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

    pub async fn send_client_error(
        &mut self,
        response_line: &str,
        response_content: &str,
    ) -> Error {
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
const URI_TOO_LONG: &str = "414 URI Too Long"; // 414
const REQUEST_TOO_LARGE: &str = "431 Request Entity Too Large"; // 431
