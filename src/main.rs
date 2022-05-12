#![allow(warnings)]
use anyhow::{anyhow, bail, Result};
use config::HpsConfig;
use once_cell;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{env, net::SocketAddr, process, sync::Arc};
use tokio::{
    self, fs,
    net::{TcpListener, TcpStream},
};
use tracing::{error, info, warn};
use tracing_subscriber;

use crate::config::exit_if_err;
use crate::config::CONFIG;

mod adapter;
#[macro_use]
mod config;
mod server;

mod client;

pub async fn run() -> Result<()> {
    if CONFIG.verbose {
        info!("hps_config = {CONFIG:#?}");
    }

    let mut server = server::create_server().await?;

    info!(
        "server started at: {}:{}. Press Ctrl-C to stop.",
        CONFIG.server_addr, CONFIG.server_port
    );

    server::run_server(&mut server).await?;

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
