// get matched server from HpsConfig.
// create client from matched server address.
// establish bridge: client <-> hps <-> server

use crate::config::HpsConfig;
use anyhow::{Context, Result};
use std::{net::SocketAddr, sync::Arc};
use tokio::{io::AsyncWriteExt, net::TcpStream, select};

pub async fn connect_to_server(config: &HpsConfig, path: &str) -> Result<TcpStream> {
    let addr = config
        .match_path(path)
        .with_context(|| format!("path: {:?} has no match", path))?;
    let stream = TcpStream::connect(addr).await?;

    Ok(stream)
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
}

impl Bridge {
    pub async fn new(
        config: Arc<HpsConfig>,
        client: TcpStream,
        client_addr: SocketAddr,
    ) -> Result<Self> {
        let path = "";
        let server = connect_to_server(&config, path).await?;

        Ok(Self {
            config,
            client,
            server,
            client_addr,
            client_read: 0,
            server_read: 0,
            buffer: Vec::new(),
        })
    }

    pub fn bytes_read(&self) -> usize {
        self.client_read
    }

    pub fn bytes_write(&self) -> usize {
        self.server_read
    }

    pub async fn run(&mut self) -> Result<()> {
        Ok(())
    }
}
