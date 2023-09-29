use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

fn default_domain() -> String {
    "".to_string()
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MappingConfig {
    pub name: String,
    pub mode: String,
    pub local_addr: Option<SocketAddr>,
    #[serde(default="default_domain")]
    pub domain: String,
}

impl MappingConfig {
    pub fn new(name: String, mode: String, domain: String) -> Self {
        MappingConfig {
            name,
            mode,
            local_addr: None,
            domain,
        }
    }
}
