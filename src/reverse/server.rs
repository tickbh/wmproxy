use std::{net::SocketAddr};
use serde::{Deserialize, Serialize};
use super::{LocationConfig, UpstreamConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub server_name: String,
    pub root: Option<String>,
    pub cert: Option<String>,
    pub key: Option<String>,
    #[serde(default = "Vec::new")]
    pub location: Vec<LocationConfig>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,
}

impl ServerConfig {
    /// 将配置参数提前共享给子级
    pub fn copy_to_child(&mut self) {
        for l in &mut self.location {
            l.upstream.append(&mut self.upstream.clone());
            if l.root.is_none() && self.root.is_some() {
                l.root = self.root.clone();
                if let Some(file_server) = &mut l.file_server {
                    if file_server.root.is_none() && self.root.is_some() {
                        file_server.root = self.root.clone();
                    }
                    if file_server.prefix.is_empty() {
                        file_server.set_prefix(l.rule.clone());
                    }
                }
            }
        }
    }
}
