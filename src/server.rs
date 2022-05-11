use crate::client::Bridge;
use crate::config::HpsConfig;
use anyhow::{anyhow, bail, Context, Result};
use std::{future::Future, net::SocketAddr, str, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tracing::{error, info, warn};

pub const DEFAULT_CLIENT_READ_BUFF: usize = 1024;

pub async fn create_server(config: Arc<HpsConfig>) -> Result<TcpListener> {
    let server = TcpListener::bind(&config.server_addr).await?;

    Ok(server)
}

pub async fn handle_client(
    config: Arc<HpsConfig>,
    mut client: TcpStream,
    addr: SocketAddr,
) -> Result<()> {
    // let bridge = Bridge::new()

    Ok(())
}

pub async fn handle_error(future: impl Future<Output = Result<()>>) {
    if let Err(err) = future.await {
        error!("handle client error: {err}");
    }
}

pub fn create_bad_request_response() -> String {
    format!(
        "\
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
    </html>"
    )
}

pub async fn run_server(server: &mut TcpListener, config: Arc<HpsConfig>) -> Result<()> {
    loop {
        let (client, client_addr) = match server.accept().await {
            Ok(client) => client,
            Err(err) => {
                warn!("error");
                continue;
            }
        };

        info!("got request from: {}", client_addr);

        let task = handle_client(config.clone(), client, client_addr);

        tokio::spawn(handle_error(task));
    }

    Ok(())
}
