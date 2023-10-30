use std::{net::SocketAddr, time::{Instant, Duration}};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DurationSeconds;
use super::{LocationConfig, UpstreamConfig};

fn default_bind_mode() -> String {
    "tcp".to_string()
}

fn connect_timeout() -> Duration {
    Duration::from_secs(180)
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
    #[serde_as(as = "DurationSeconds<u64>")]
    #[serde(default = "connect_timeout")]
    pub timeout: Duration,
    
    #[serde(default = "Vec::new")]
    pub headers: Vec<Vec<String>>,
    #[serde(default = "Vec::new")]
    pub location: Vec<LocationConfig>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,
}

impl ServerConfig {
    /// 将配置参数提前共享给子级
    pub fn copy_to_child(&mut self) {
        for l in &mut self.location {
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
                }
            }
        }
    }
}
