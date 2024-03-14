use async_trait::async_trait;
use std::{io, net::{IpAddr, Ipv4Addr, SocketAddr}, sync::Arc};

use crate::{
    core::{AppTrait, Listeners, Service, ShutdownWatch, Stream, WrapListener},
    ProxyResult,
};

use super::{HttpConfig, ServerConfig};

pub struct HttpApp {
    pub config: HttpConfig,
    pub http_servers: Vec<Arc<ServerConfig>>,
}

impl HttpApp {
    pub fn new(config: HttpConfig) -> Self {
        let http_servers = config
            .server
            .clone()
            .into_iter()
            .map(|f| Arc::new(f))
            .collect();
        Self {
            config,
            http_servers,
        }
    }

    pub fn build_services(mut config: HttpConfig) -> ProxyResult<Service<Self>> {
        let listeners = config.bind_app()?;
        let app = Self::new(config);
        Ok(Service::with_listeners(
            "center_app".to_string(),
            listeners,
            app,
        ))
    }
}

#[async_trait]
impl AppTrait for HttpApp {
    async fn process_new(
        self: &Arc<Self>,
        session: Stream,
        _shutdown: &ShutdownWatch,
    ) -> Option<Stream> {
        println!("aaaaaaaaaaaaaaa");
        let local_addr = session.listen_addr();
        // log::trace!("反向代理:{}收到客户端连接: {}->{}", if self.http_tlss[index] { "https" } else { "http" }, addr,self.http_listeners[index].local_addr()?);
        let mut local_servers = vec![];
        
        let addr = session.client_addr();
        for s in &self.http_servers {
            if !(*s).bind_addr.contains(local_addr.port()) && !(*s).bind_ssl.contains(local_addr.port()) {
                continue;
            }
            local_servers.push(s.clone());
        }
        let _ = HttpConfig::process(local_servers, session, addr).await;
        None
    }

    async fn ready_init(&mut self) -> io::Result<()> {
        Ok(())
    }
}
