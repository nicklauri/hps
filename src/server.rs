use crate::adapter::Adapter;
use crate::config::CONFIG;
use anyhow::{Context, Result};
use std::{future::Future, net::SocketAddr};
use tokio::net::{TcpListener, TcpStream};
use tracing::{info, warn};

pub async fn create_server() -> Result<TcpListener> {
    let server_addr = format!("{}:{}", CONFIG.server_addr, CONFIG.server_port);

    let server = TcpListener::bind(&server_addr)
        .await
        .with_context(|| format!("bind server failed: server_addr={}", server_addr))?;

    Ok(server)
}

pub async fn handle_client(client: TcpStream, addr: SocketAddr) -> Result<()> {
    Adapter::new(client, addr).run().await
}

pub async fn handle_error(future: impl Future<Output = Result<()>>, _client_addr: SocketAddr) {
    if let Err(err) = future.await {
        if let Some(0) = err.downcast_ref::<usize>() {
            // Reached EOF;
            return;
        }

        if CONFIG.verbose {
            warn!("{err:?}");

            err.chain().skip(1).for_each(|e| {
                warn!("caused by: {e}");
            });
        } else {
            warn!("{err}");
        }
    }
}

#[allow(dead_code)]
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

pub async fn run_server(server: &mut TcpListener) -> Result<()> {
    loop {
        let (client, client_addr) = match server.accept().await {
            Ok(client) => client,
            Err(err) => {
                warn!("{err:?}");
                continue;
            }
        };

        if CONFIG.verbose {
            info!("got request from: {}", client_addr);
        }

        let task = handle_client(client, client_addr);

        tokio::spawn(handle_error(task, client_addr));
    }
}
