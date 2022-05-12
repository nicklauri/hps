use crate::client::{self, Bridge};
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
    let server_addr = format!("{}:{}", config.server_addr, config.server_port);

    let server = TcpListener::bind(&server_addr)
        .await
        .with_context(|| format!("bind server failed: server_addr={}", server_addr))?;

    Ok(server)
}

pub async fn handle_client(
    config: Arc<HpsConfig>,
    client: TcpStream,
    addr: SocketAddr,
) -> Result<()> {
    match client::build_bridge(config, client, addr).await? {
        Some(bridge) => bridge.run().await,
        None => Ok(()),
    }
}

pub async fn handle_error(future: impl Future<Output = Result<()>>, client_addr: SocketAddr) {
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

        if config.verbose {
            info!("got request from: {}", client_addr);
        }

        let task = handle_client(config.clone(), client, client_addr);

        tokio::spawn(handle_error(task, client_addr));
    }

    Ok(())
}
