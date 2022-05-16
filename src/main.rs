#![allow(warnings)]
use crate::config::CONFIG;
use anyhow::Result;
use std::process;
use tokio;
use tracing::{error, info};
use tracing_subscriber;

mod config;
mod server;
mod utils;

pub async fn run() -> Result<()> {
    if CONFIG.verbose {
        info!("hps_config = {:#?}", &*CONFIG);
    }

    info!(
        "server started at: {}:{}. Press Ctrl-C to stop.",
        CONFIG.server_addr, CONFIG.server_port
    );

    server::run().await?;

    Ok(())
}

extern "system" {
    fn GetStdHandle(handle: i32) -> usize;
    fn SetConsoleMode(console_handle: usize, console_mode: u32) -> i32;
}

fn setup_color() {
    // On Windows, we have two choice to setup console to use ANSI color escape codes:
    //      WinAPI: SetConsoleMode(GetStdHandle(-11), 7)
    // If we use Python, simply call os.system('') or os.system('color') and it's done.
    const STDOUT_HANDLE: i32 = -11;
    unsafe {
        SetConsoleMode(GetStdHandle(STDOUT_HANDLE), 7);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    if cfg!(target_os = "windows") {
        setup_color();
    }

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
