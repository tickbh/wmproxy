use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MappingConfig {
    pub name: String,
    pub mode: String,
    pub local_addr: Option<SocketAddr>,
    pub domain: Option<String>,
}

impl MappingConfig {
    pub fn new(name: String, mode: String, domain: String) -> Self {
        MappingConfig {
            name,
            mode,
            local_addr: None,
            domain: Some(domain),
        }
    }
}
