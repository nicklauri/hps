use anyhow::{anyhow, bail, Error, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json;
use std::env;
use tokio::fs;
use tracing::error;

pub const DEFAULT_BUFFER_SIZE: usize = 1024 * 8;
pub const MAX_NUMBERS_OF_HEADERS: usize = 100;

pub static CONFIG: Lazy<HpsConfig> = Lazy::new(HpsConfig::new);

#[derive(Debug, Serialize, Deserialize)]
pub struct HpsConfig {
    pub server_addr: String,
    pub server_port: u16,
    pub paths: Vec<Matcher>,

    #[serde(default)]
    pub verbose: bool,

    #[serde(default = "HpsConfig::get_default_bridge_buffer_size")]
    pub buffer_size: usize,
}

impl HpsConfig {
    pub fn new() -> Self {
        let config_file_path = exit_if_err!(env::args()
            .nth(1)
            .ok_or_else(|| anyhow!("no config file provided.")));

        let config_file_content = match std::fs::read_to_string(&config_file_path) {
            Ok(content) => content,
            Err(err) => {
                error!(
                    "parse_config_from_args: can't read content from path: {:?}",
                    config_file_path
                );
                std::process::exit(1);
            }
        };

        let hps_config = exit_if_err!(serde_json::from_str::<HpsConfig>(&config_file_content));

        hps_config.format_server_addr()
    }

    pub fn match_path(&self, path: &str) -> Option<usize> {
        self.paths.iter().position(|m| m.is_match(path))
    }

    pub fn get_default_bridge_buffer_size() -> usize {
        DEFAULT_BUFFER_SIZE
    }

    fn format_server_addr(mut self) -> Self {
        let iter = self.paths.iter_mut();

        for matcher in iter {
            let server_addr = if matcher.server_ip.is_empty() {
                &self.server_addr
            } else {
                &matcher.server_ip
            };

            matcher.server_addr = format!("{}:{}", server_addr, matcher.server_port);
        }

        self
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Matcher {
    starts_with: String,
    server_port: u16,

    #[serde(default)]
    server_ip: String,

    // private
    #[serde(default)]
    server_addr: String,
}

impl Matcher {
    pub fn is_match(&self, path: &str) -> bool {
        path.starts_with(&self.starts_with)
    }

    pub fn match_and_get_addr(&self, path: &str) -> Option<&str> {
        if self.is_match(path) {
            Some(&self.server_addr)
        } else {
            None
        }
    }

    pub fn server_addr(&self) -> &str {
        &self.server_addr
    }

    pub fn matcher(&self) -> &str {
        &self.starts_with
    }
}

macro_rules! exit_if_err {
    ($e:expr) => {
        match $e {
            Ok(result) => result,
            Err(err) => {
                let err: anyhow::Error = err.into();

                error!("config: {}", err);

                err.chain().skip(1).for_each(|e| {
                    error!("caused by: {e}");
                });

                std::process::exit(0);
            }
        }
    };
}
pub(crate) use exit_if_err;
