use async_trait::async_trait;
use tokio::sync::Mutex;
use std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use crate::{
    core::{AppTrait, Listeners, Service, ShutdownWatch, Stream, WrapListener},
    ProxyResult,
};

use super::{HttpConfig, ServerConfig, StreamConfig};

pub struct StreamApp {
    pub config: StreamConfig,
    pub stream_config: Arc<Mutex<StreamConfig>>,
}

impl StreamApp {
    pub fn new(config: StreamConfig) -> Self {
        let stream_config = Arc::new(Mutex::new(config.clone()));
        Self {
            config,
            stream_config,
        }
    }

    pub fn build_services(mut config: StreamConfig) -> ProxyResult<Service<Self>> {
        let listeners = config.bind_tcp_app()?;
        let app = Self::new(config);
        Ok(Service::with_listeners(
            "stream_app".to_string(),
            listeners,
            app,
        ))
    }
}

#[async_trait]
impl AppTrait for StreamApp {
    async fn process_new(
        self: &Arc<Self>,
        session: Stream,
        _shutdown: &ShutdownWatch,
    ) -> Option<Stream> {
        println!("aaaaaaaaaaaaaaa");

        let addr = session.client_addr();
        // log::trace!("反向代理:{}收到客户端连接: {}->{}", "stream", addr, self.stream_listeners[index].local_addr()?);
        let data = self.stream_config.clone();
        let local_addr = session.listen_addr().clone();
        tokio::spawn(async move {
            let _ = StreamConfig::process(data, local_addr, session, addr).await;
        });
        None
    }

    async fn ready_init(&mut self) -> io::Result<()> {
        Ok(())
    }
}
