#![allow(warnings)]
use std::{env, net::SocketAddr, sync::Arc};

use anyhow::{anyhow, bail, Result};
use config::HpsConfig;
use httparse;
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::{
    self, fs,
    net::{TcpListener, TcpStream},
};
use tracing::{error, info, warn};
use tracing_subscriber;

mod client;
mod config;
mod server;

pub async fn run() -> Result<()> {
    let config = Arc::new(config::parse_config_from_args().await?);

    info!(hps_config = ?config, "run hps with config");

    let mut server = server::create_server(config.clone()).await?;

    info!(
        "server started at: {} . Press Ctrl-C to stop.",
        &config.server_addr
    );

    server::run_server(&mut server, config).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    if let Err(err) = run().await {
        error!("error: {err}");
    }
}
