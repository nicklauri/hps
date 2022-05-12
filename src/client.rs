// get matched server from HpsConfig.
// create client from matched server address.
// establish bridge: client <-> hps <-> server

use crate::{
    config::{self, HpsConfig, CONFIG},
    server,
};
use anyhow::{bail, Context, Result};
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    select,
};
use tracing::{info, warn};

pub async fn build_bridge(
    mut client: TcpStream,
    client_addr: SocketAddr,
) -> Result<Option<Bridge>> {
    let mut buff = [0u8; config::DEFAULT_BUFFER_SIZE];
    let mut bytes_read = 0;

    loop {
        let amount = client
            .read(&mut buff[bytes_read..])
            .await
            .context("initial read for client failed")?;

        if amount == 0 && bytes_read == 0 {
            client.shutdown().await?;

            warn!("initial read: client {client_addr} send zero bytes!");
            return Ok(None);
        }

        let req = &buff[..amount];

        let path = get_path(req);
        let matched_matcher_idx = path.and_then(|p| CONFIG.match_path(p));

        if let Some(matcher_idx) = matched_matcher_idx {
            let server_addr = CONFIG.paths[matcher_idx].server_addr();
            let path = path.unwrap();
            let matcher = CONFIG.paths[matcher_idx].matcher();
            info!("client={client_addr}, server={server_addr}: matched path {path:?} against {matcher:?}");

            let bridge = Bridge::new(client, client_addr, matcher_idx, req).await?;

            return Ok(Some(bridge));
        } else if amount == 0 || path.is_some() {
            // request ended but no path matched, response with bad request.
            if CONFIG.verbose {
                warn!("client={client_addr}: mismatched path {path:?}");
            }

            let response = server::create_bad_request_response();
            client.write_all(response.as_bytes()).await?;
            client.shutdown().await?;
            break;
        }
    }

    Ok(None)
}

fn get_path(src: &[u8]) -> Option<&str> {
    if src.contains(&b'\n') {
        let path = src.split(|&ch| ch == b' ').nth(1)?;
        std::str::from_utf8(path).ok()
    } else {
        None
    }
}

#[derive(Debug)]
pub struct Bridge {
    client: TcpStream,
    server: TcpStream,
    client_addr: SocketAddr,
    server_addr: String,
    client_read: u64,
    buffer: Vec<u8>,
    matcher_idx: usize,
}

impl Bridge {
    pub async fn new(
        client: TcpStream,
        client_addr: SocketAddr,
        matcher_idx: usize,
        request_bytes: &[u8],
    ) -> Result<Self> {
        let server_addr = CONFIG.paths[matcher_idx].server_addr();

        let mut server = TcpStream::connect(server_addr).await?;

        if CONFIG.verbose {
            info!("established connection: client={client_addr} <=> server={server_addr}");
        }

        server.write_all(request_bytes).await?;

        let buffer_size = CONFIG.buffer_size;

        Ok(Self {
            client,
            server,
            client_addr,
            server_addr: server_addr.to_string(),
            client_read: request_bytes.len() as _,
            buffer: vec![0u8; buffer_size],
            matcher_idx,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        let (client_read, server_read) =
            io::copy_bidirectional(&mut self.client, &mut self.server).await?;

        info!(
            "client={}; server={}: read={} bytes; write={} bytes",
            self.client_addr,
            self.server_addr,
            client_read + self.client_read,
            server_read
        );

        Ok(())
    }
}
