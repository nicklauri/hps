use crate::utils;
use anyhow::{anyhow, Result};
use hyper::Uri;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json;
use std::env;
use tokio::net::TcpStream;
use tracing::{error, info};

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
        let config_file_path = exit_if_err!(env::args().nth(1).ok_or_else(|| anyhow!("no config file provided.")));

        let config_file_content = match std::fs::read_to_string(&config_file_path) {
            Ok(content) => content,
            Err(_) => {
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

    pub fn match_path<'a, 'b>(&'a self, path: &'b str) -> Option<&'a Matcher> {
        self.paths.iter().find(|m| m.is_match(path))
    }

    pub fn get_default_bridge_buffer_size() -> usize {
        DEFAULT_BUFFER_SIZE
    }

    fn format_server_addr(mut self) -> Self {
        let iter = self.paths.iter_mut();

        for matcher in iter {
            let mut server_addr = if matcher.server_ip.is_empty() {
                self.server_addr.trim()
            } else {
                matcher.server_ip.trim()
            };

            // Prevent TcpStream::connect failed when server is listening on 0.0.0.0
            if server_addr == "0.0.0.0" {
                server_addr = "127.0.0.1";
            }

            matcher.server_addr = format!("{}:{}", server_addr, matcher.server_port);
        }

        self
    }

    pub fn get_uri(&self, uri: &Uri) -> Option<Uri> {
        self.paths.iter().find_map(|p| p.match_uri(uri).ok().flatten())
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
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

    pub fn server_addr(&self) -> &str {
        &self.server_addr
    }

    pub fn match_uri(&self, uri: &Uri) -> Result<Option<Uri>> {
        if self.is_match(uri.path()) {
            let new_uri = format!(
                "http://{}{}",
                self.server_addr,
                uri.path_and_query().map(|p| p.to_string()).unwrap_or_default()
            );

            info!("matched URI from: {} => {}", uri, new_uri);

            return Ok(Some(new_uri.parse()?));
        }

        Ok(None)
    }

    pub async fn create_connection(&self) -> Result<TcpStream> {
        utils::connect(&self.server_addr).await
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
