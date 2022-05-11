#![allow(warnings)]
use std::{env, net::SocketAddr};

use anyhow::{anyhow, bail, Result};
use config::HpsConfig;
use httparse;
use tokio::{self, fs, net::{TcpListener, TcpStream}};
use tracing::{info, warn, error};
use tracing_subscriber;
use serde_json;
use serde::{Deserialize, Serialize};

mod client;
mod config;
mod server;

pub async fn run() -> Result<()> {
    let config = config::parse_config_from_args().await?;

    info!(hps_config = ?config, "run hps with config");

    let mut server = server::create_server(&config).await?;

    info!("server started at: {} . Press Ctrl-C to stop.", config.server_addr);

    server::run_server(&mut server).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    if let Err(err) = run().await {
        error!("error: {err}");
    }
}
