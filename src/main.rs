#![allow(warnings)]
use anyhow::{bail, Result};
use httparse;
use tokio;
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Hello world");

    Ok(())
}
