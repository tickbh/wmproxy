use std::{collections::HashSet, sync::Arc, net::SocketAddr};

use serde::{Serialize, Deserialize};
use tokio::{net::TcpListener, sync::Mutex, io::{AsyncRead, AsyncWrite, copy_bidirectional}};
use wenmeng::Server;

use crate::{ProxyResult, Helper, HealthCheck};

use super::{ServerConfig, UpstreamConfig, ReverseHelper};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    #[serde(default = "Vec::new")]
    pub server: Vec<ServerConfig>,
    #[serde(default = "Vec::new")]
    pub upstream: Vec<UpstreamConfig>,
}


impl StreamConfig {
    pub fn new() -> Self {
        StreamConfig {
            server: vec![],
            upstream: vec![],
        }
    }
    
    /// 将配置参数提前共享给子级
    pub fn copy_to_child(&mut self) {
        for server in &mut self.server {
            server.upstream.append(&mut self.upstream.clone());
            server.copy_to_child();
        }
    }

    pub async fn bind(
        &mut self,
    ) -> ProxyResult<Vec<TcpListener>> {
        let mut listeners = vec![];
        let mut bind_port = HashSet::new();
        for value in &self.server.clone() {

            if bind_port.contains(&value.bind_addr.port()) {
                continue;
            }
            bind_port.insert(value.bind_addr.port());
            let listener = Helper::bind(value.bind_addr).await?;
            listeners.push(listener);
        }

        Ok(listeners)
    }


    pub async fn process<T>(data: Arc<Mutex<StreamConfig>>, local_addr: SocketAddr, mut inbound: T, addr: SocketAddr) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin + std::marker::Send + 'static,
    {
        
        let value = data.lock().await;
        for (_, s) in value.server.iter().enumerate() {
            if s.bind_addr.port() == local_addr.port() {
                let addr = ReverseHelper::get_upstream_addr(&s.upstream, "")?;
                let mut connect = HealthCheck::connect(&addr).await?;
                copy_bidirectional(&mut inbound, &mut connect).await?;
                break;
            }
        };
        Ok(())
    }
}
