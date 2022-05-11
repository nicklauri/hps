// get matched server from HpsConfig.
// create client from matched server address.
// establish bridge: client <-> hps <-> server

use crate::{
    config::{self, HpsConfig},
    server,
};
use anyhow::{bail, Context, Result};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    select,
};
use tracing::info;

pub async fn build_bridge(
    config: Arc<HpsConfig>,
    mut client: TcpStream,
    client_addr: SocketAddr,
) -> Result<Option<Bridge>> {
    let mut buff = [0u8; config::DEFAULT_BUFFER_SIZE];
    let mut bytes_read = 0;
    let mut request_bytes = Vec::new();

    loop {
        let amount = client
            .read(&mut buff)
            .await
            .context("initial read for client failed")?;

        if amount == 0 && bytes_read == 0 {
            client.shutdown().await?;

            bail!("initial read: client {client_addr} send zero bytes!");
        }

        request_bytes.extend_from_slice(&buff[..amount]);

        if let Some(matcher_idx) = match_from_bytes(&config, &request_bytes) {
            let bridge =
                Bridge::new(config, client, client_addr, matcher_idx, &request_bytes).await?;

            return Ok(Some(bridge));
        }

        if amount == 0 {
            // request ended but no path matched, response with bad request.
            let response = server::create_bad_request_response();
            client.write_all(response.as_bytes()).await?;
            client.shutdown().await?;
            break;
        }
    }

    Ok(None)
}

fn match_from_bytes<'a>(config: &HpsConfig, src: &'a [u8]) -> Option<usize> {
    if src.contains(&b'\n') {
        let path = src.split(|&ch| ch == b' ').nth(1)?;
        let path = std::str::from_utf8(src).ok()?;

        config.match_path(path)
    } else {
        None
    }
}

#[derive(Debug)]
pub struct Bridge {
    config: Arc<HpsConfig>,
    client: TcpStream,
    server: TcpStream,
    client_addr: SocketAddr,
    client_read: usize,
    server_read: usize,
    buffer: Vec<u8>,
    matcher_idx: usize,
}

impl Bridge {
    pub async fn new(
        config: Arc<HpsConfig>,
        client: TcpStream,
        client_addr: SocketAddr,
        matcher_idx: usize,
        request_bytes: &[u8],
    ) -> Result<Self> {
        let server_addr = config.paths[matcher_idx].server_addr();

        let mut server = TcpStream::connect(server_addr).await?;

        info!("established connection: client={client_addr} <=> server={server_addr}");

        server.write_all(request_bytes).await?;

        let buffer_size = config.buffer_size;

        Ok(Self {
            config,
            client,
            server,
            client_addr,
            client_read: request_bytes.len(),
            server_read: 0,
            buffer: vec![0u8; buffer_size],
            matcher_idx,
        })
    }

    pub fn bytes_read(&self) -> usize {
        self.client_read
    }

    pub fn bytes_write(&self) -> usize {
        self.server_read
    }

    pub async fn run(mut self) -> Result<()> {
        select! {
            _ = self.client.readable() => {
                let amount = self.client.read(&mut self.buffer).await?;

                self.server.write_all(&self.buffer[..amount]).await?;
            }
            _ = self.server.readable() => {
                let amount = self.server.read(&mut self.buffer).await?;

                self.client.write_all(&self.buffer[..amount]).await?;
            }
        }

        Ok(())
    }
}
