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

        let server_addr_errs = hps_config
            .paths
            .iter()
            .map(|p| (p.server_addr(), str::parse::<Uri>(p.server_addr())));

        let mut is_err = false;

        for (addr, urires) in server_addr_errs {
            match urires {
                Ok(uri) => {
                    if uri.path() != "/" {
                        is_err = true;
                        error!("ERR: addr={addr} server address shouldn't contain any path.");
                    }
                }
                Err(err) => {
                    is_err = true;
                    error!("ERR: addr={}: {:?}", addr, err);
                }
            }
        }

        if is_err {
            std::process::exit(1);
        }

        hps_config
    }

    pub fn match_path<'a, 'b>(&'a self, path: &'b str) -> Option<&'a Matcher> {
        self.paths.iter().find(|m| m.is_match(path))
    }

    pub fn get_default_bridge_buffer_size() -> usize {
        DEFAULT_BUFFER_SIZE
    }

    pub fn get_uri(&self, uri: &Uri) -> Option<Uri> {
        self.paths.iter().find_map(|p| p.match_uri(uri).ok().flatten())
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Matcher {
    #[serde(default)]
    is_prefix: bool,
    starts_with: String,

    // TODO: server_addr should not have any path and follow format: ^http[s]://(host:)$
    // Will verify this later, keep it simple for now.
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
            let path = uri.path_and_query().map(|p| p.to_string()).unwrap_or_default();

            let path = path.split_at(if self.is_prefix { self.starts_with.len() } else { 0 }).1;

            let new_uri = format!("{}{}", self.server_addr, path);

            return Ok(Some(new_uri.parse()?));
        }

        Ok(None)
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
