#![allow(warnings)]
use anyhow::{bail, Result};
use httparse;
use tokio;
use tracing::{info, warn};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let max_concurrent_conn = 10;
    info!("incomming request from localhost:3334");
    warn!(max_concurrent_conn, "reached max concurrent connection:");

    Ok(())
}
