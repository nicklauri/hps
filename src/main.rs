#![allow(warnings)]
use anyhow::{anyhow, bail, Result};
use config::HpsConfig;
use httparse;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{env, net::SocketAddr, process, sync::Arc};
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

    info!("hps_config = {config:#?}");

    let mut server = server::create_server(config.clone()).await?;

    info!(
        "server started at: {}:{}. Press Ctrl-C to stop.",
        config.server_addr, config.server_port
    );

    server::run_server(&mut server, config).await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let result = ctrlc::set_handler(|| {
        info!("server is shutting down");

        process::exit(0);
    });

    if let Err(err) = result {
        error!("setup Ctrl-C failed: {}", err);
        return;
    }

    if let Err(err) = run().await {
        error!("{err}");
        err.chain().skip(1).for_each(|e| error!("caused by: {e}"));
    }
}
