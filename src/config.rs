use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json;
use std::env;
use tokio::fs;

pub const DEFAULT_BRIDGE_BUFFER_SIZE: usize = 1024 * 8;

#[derive(Debug, Serialize, Deserialize)]
pub struct HpsConfig {
    pub server_addr: String,
    pub server_port: u16,
    pub paths: Vec<Matcher>,

    #[serde(default = "HpsConfig::get_default_bridge_buffer_size")]
    pub buffer_size: usize,
}

impl HpsConfig {
    pub fn match_path(&self, path: &str) -> Option<&str> {
        self.paths
            .iter()
            .filter_map(|m| m.match_and_get_addr(path))
            .next()
    }

    pub fn get_default_bridge_buffer_size() -> usize {
        DEFAULT_BRIDGE_BUFFER_SIZE
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Matcher {
    starts_with: String,
    forward_to_port: u16,

    // private
    #[serde(default)]
    formatted_add: String,
}

impl Matcher {
    pub fn is_match(&self, path: &str) -> bool {
        path.starts_with(&self.starts_with)
    }

    pub fn match_and_get_addr(&self, path: &str) -> Option<&str> {
        if self.is_match(path) {
            Some(&self.formatted_add)
        } else {
            None
        }
    }
}

pub async fn parse_config_from_args() -> Result<HpsConfig> {
    let config_file = env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("no config file provided."))?;

    let config_file_content = fs::read_to_string(&config_file).await?;

    let hps_config = serde_json::from_str(&config_file_content)?;

    Ok(hps_config)
}
