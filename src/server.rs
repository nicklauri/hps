use std::{net::SocketAddr, future::Future, str};
use tokio::{net::{TcpListener, TcpStream}, io::{AsyncReadExt, AsyncWriteExt}};
use anyhow::{anyhow, bail, Result, Context};
use tracing::{warn, info, error};
use crate::config::HpsConfig;

pub const DEFAULT_CLIENT_READ_BUFF: usize = 1024;

pub async fn create_server(config: &HpsConfig) -> Result<TcpListener> {
    let server = TcpListener::bind(&config.server_addr).await?;
    
    Ok(server)
}

pub async fn handle_client(mut client: TcpStream, addr: SocketAddr) -> Result<()> {
    let buf = &mut [0u8; DEFAULT_CLIENT_READ_BUFF];

    let amount = client.read(buf).await?;
    if amount == 0 {
        return Ok(());
    }

    let buf = &mut buf[..amount];

    let path = buf.split(|&ch| ch == b' ').nth(1).context("can't find request path")?;
    let path = str::from_utf8(path)?;

    info!("client [{addr}] request path: {path}");

    let response = create_bad_request_response();
    let response = response.as_bytes();

    client.write_all(&response).await?;

    Ok(())
}

pub async fn handle_error(future: impl Future<Output = Result<()>>) {
    if let Err(err) = future.await {
        error!("handle client error: {err}");
    }
}

pub fn create_bad_request_response() -> String {
    format!("\
    HTTP/1.1 400 Bad Request\
    server: hps\
    connection: closed\
    \n\n\
    <!DOCTYPE>
    <html>
    <head>
        <title>Bad request</title>
    </head>
    <body><pre>
        Bad request!
    </pre></body>
    </html>")
}

pub async fn run_server(server: &mut TcpListener) -> Result<()> {
    loop {
        let (client, client_addr) = match server.accept().await {
            Ok(client) => client,
            Err(err) => {
                warn!("error");
                continue;
            }
        };

        info!("got request from: {}", client_addr);

        let task = handle_client(client, client_addr);

        tokio::spawn(handle_error(task));
    }

    Ok(())
}