use std::{net::SocketAddr, time::{Duration}};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use crate::ConfigDuration;

use super::{LocationConfig, UpstreamConfig, common::CommonConfig};

fn default_bind_mode() -> String {
    "tcp".to_string()
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub server_name: String,
    pub root: Option<String>,
    pub cert: Option<String>,
    pub key: Option<String>,

    #[serde(default = "default_bind_mode")]
    pub bind_mode: String,
    
    #[serde(default = "Vec::new")]
    pub headers: Vec<Vec<String>>,
    #[serde(default = "Vec::new")]
    pub location: Vec<LocationConfig>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,

    #[serde(flatten)]
    #[serde(default = "CommonConfig::new")]
    pub comm: CommonConfig,
}

impl ServerConfig {
    /// 将配置参数提前共享给子级
    pub fn copy_to_child(&mut self) {
        for l in &mut self.location {
            l.comm.copy_from_parent(&self.comm);
            l.server_name = Some(self.server_name.clone());
            l.upstream.append(&mut self.upstream.clone());
            l.headers.append(&mut self.headers.clone());
            if l.root.is_none() && self.root.is_some() {
                l.root = self.root.clone();
                if let Some(file_server) = &mut l.file_server {
                    if file_server.root.is_none() && self.root.is_some() {
                        file_server.root = self.root.clone();
                    }
                    if file_server.prefix.is_empty() {
                        file_server.set_prefix(l.rule.clone());
                    }
                    file_server.set_common(l.comm.clone());
                }
            }
        }
    }
}
