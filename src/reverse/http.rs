
use serde::{Serialize, Deserialize};
use tokio::net::TcpListener;
use wenmeng::{FileServer, ProtResult};

use crate::ProxyResult;

use super::ServerConfig;


fn default_servers() -> Vec<ServerConfig> {
    vec![]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpConfig {
    #[serde(default = "default_servers")]
    pub server: Vec<ServerConfig>,
}

impl HttpConfig {
    pub async fn bind(&mut self) -> ProxyResult<(Vec<ServerConfig>, Vec<TcpListener>)>  {
        let mut listeners = vec![];
        let mut configs = vec![];
        for value in self.server.clone() {
            let listener = TcpListener::bind(value.bind_addr).await?;
            configs.push(value);
            listeners.push(listener);
        }
        Ok((configs, listeners))
    }
}