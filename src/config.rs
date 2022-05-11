use std::env;
use anyhow::{anyhow, bail, Result};
use serde_json;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct HpsConfig {
    pub server_addr: String,
    pub paths: Vec<Matcher>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Matcher {
    pub starts_with: String,
    pub forward_to_port: u16,
}

pub async fn parse_config_from_args() -> Result<HpsConfig> {
    let config_file = env::args().nth(1).ok_or_else(|| anyhow!("no config file provided."))?;

    let config_file_content = fs::read_to_string(&config_file).await?;

    let hps_config = serde_json::from_str(&config_file_content)?;

    Ok(hps_config)
}
