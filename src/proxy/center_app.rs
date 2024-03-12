use std::{io, sync::Arc};

use async_trait::async_trait;

use crate::{
    core::{AppTrait, Listeners, Service, ShutdownWatch, Stream}, proxy::data::ProxyData, CenterServer, CenterTrans, ProxyConfig
};

pub struct CenterApp {
    config: ProxyConfig,
    tls_client: Option<Arc<rustls::ClientConfig>>,
}

impl CenterApp {
    pub fn new(config: ProxyConfig) -> Self {
        let tls_client = config.get_tls_request().ok();
        Self { config, tls_client }
    }

    pub fn build_services(self, listeners: Listeners) -> Service<Self> {
        Service::with_listeners("center_app".to_string(), listeners, self)
    }
}

#[async_trait]
impl AppTrait for CenterApp {
    async fn process_new(
        self: &Arc<Self>,
        session: Stream,
        _shutdown: &ShutdownWatch,
    ) -> Option<Stream> {
        println!("aaaaaaaaaaaaaaa");
        if let Some(server) = self.config.server.clone() {
            let mut server = CenterTrans::new(server, self.config.domain.clone(), self.tls_client.clone());
            let _ = server.serve(session).await;
        } else {
            let mut server = CenterServer::new(self.config.clone());
            let _ = server.serve(session).await;
            ProxyData::cache_server(server);
            // self.center_servers.push(server);
            // let _ = self.center_servers.last_mut().unwrap().serve(inbound).await;
        }
        None
    }

    async fn ready_init(&mut self) -> io::Result<()> {
        // if let Some(config) = &self.config {
        //     match config.try_connect_center_client().await {
        //         Ok((client_config, center_client)) => {
        //             self.client_config = client_config;
        //             self.center_client = center_client;
        //         }
        //         Err(_) => {
        //             return Err(io::Error::new(
        //                 io::ErrorKind::Other,
        //                 "connect to center failed",
        //             ));
        //         }
        //     }
        // }
        Ok(())
    }
}
