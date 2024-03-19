use std::{io, sync::Arc};

use async_trait::async_trait;

use crate::{
    core::{AppTrait, Listeners, Service, ShutdownWatch, Stream, WrapListener}, proxy::data::ProxyData, CenterServer, CenterTrans, ProxyConfig, ProxyResult
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

    pub fn build_services(config: ProxyConfig) -> ProxyResult<Service<Self>> {
        let app = Self::new(config);
        let mut listeners = Listeners::new();
        let mut wrap = WrapListener::new(app.config.center_addr.unwrap().0).expect("ok");
        if app.config.tc {
            let accepter = app.config.get_tls_accept()?;
            wrap.accepter = Some(crate::core::WrapTlsAccepter::with_accepter(accepter));
        }
        listeners.add(wrap);
        Ok(Service::with_listeners("center_app".to_string(), listeners, app))
    }
}

#[async_trait]
impl AppTrait for CenterApp {
    async fn process_new(
        self: &Arc<Self>,
        session: Stream,
        _shutdown: &ShutdownWatch,
    ) -> Option<Stream> {
        if let Some(server) = self.config.server.clone() {
            let mut server = CenterTrans::new(server, self.config.domain.clone(), self.tls_client.clone());
            let _ = server.serve(session).await;
        } else {
            let mut server = CenterServer::new(self.config.clone());
            let _ = server.serve(session).await;
            ProxyData::cache_server(server);
        }
        None
    }

    async fn ready_init(&mut self) -> io::Result<()> {
        Ok(())
    }
}
