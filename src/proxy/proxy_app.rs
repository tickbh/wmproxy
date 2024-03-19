use async_trait::async_trait;
use rustls::ClientConfig;
use std::{io, net::IpAddr, sync::Arc};
use webparse::BinaryMut;

use crate::{
    core::{AppTrait, Listeners, Service, ShutdownWatch, Stream, WrapListener},
    error::ProxyTypeResult,
    CenterClient, ConfigHeader, Flag, ProxyConfig, ProxyError, ProxyHttp, ProxyResult, ProxySocks5,
};

pub struct ProxyApp {
    flag: Flag,
    username: Option<String>,
    password: Option<String>,
    udp_bind: Option<IpAddr>,
    headers: Option<Vec<ConfigHeader>>,
    config: ProxyConfig,
    client_config: Option<Arc<ClientConfig>>,
    center_client: Option<CenterClient>,
}

impl ProxyApp {
    pub fn new(config: ProxyConfig) -> Self {
        Self {
            flag: config.flag.clone(),
            username: config.username.clone(),
            password: config.password.clone(),
            udp_bind: config.udp_bind.clone(),
            headers: None,
            config,
            client_config: None,
            center_client: None,
        }
    }

    pub async fn deal_proxy(&self, inbound: Stream) -> ProxyTypeResult<(), Stream> {
        let (read_buf, inbound) = match self.process_http(inbound).await {
            Ok(()) => {
                return Ok(());
            }
            Err(ProxyError::Continue(buf)) => buf,
            Err(err) => return Err(err),
        };

        let _read_buf = match self.process_socks5(inbound, read_buf).await {
            Ok(()) => return Ok(()),
            Err(ProxyError::Continue(buf)) => buf,
            Err(err) => {
                log::info!("socks5代理发生错误：{:?}", err);
                return Err(err);
            }
        };
        Ok(())
    }

    async fn process_http(&self, inbound: Stream) -> ProxyTypeResult<(), Stream> {
        if self.flag.contains(Flag::HTTP) || self.flag.contains(Flag::HTTPS) {
            ProxyHttp::process(
                &self.username,
                &self.password,
                self.headers.clone(),
                inbound,
            )
            .await
        } else {
            Err(ProxyError::Continue((None, inbound)))
        }
    }

    async fn process_socks5(
        &self,
        inbound: Stream,
        buffer: Option<BinaryMut>,
    ) -> ProxyTypeResult<(), Stream> {
        if self.flag.contains(Flag::SOCKS5) {
            let mut sock =
                ProxySocks5::new(self.username.clone(), self.password.clone(), self.udp_bind);
            sock.process(inbound, buffer).await
        } else {
            Err(ProxyError::Continue((buffer, inbound)))
        }
    }

    pub fn build_services(config: ProxyConfig) -> ProxyResult<Service<Self>> {
        let bind = config.bind.unwrap().0;
        let proxy = ProxyApp::new(config);
        let mut listeners = Listeners::new();
        listeners.add(WrapListener::new(bind).expect("ok"));
        Ok(Service::with_listeners(
            "proxy_app".to_string(),
            listeners,
            proxy,
        ))
    }
}

#[async_trait]
impl AppTrait for ProxyApp {
    async fn process_new(
        self: &Arc<Self>,
        session: Stream,
        _shutdown: &ShutdownWatch,
    ) -> Option<Stream> {

        if let Some(client) = &self.center_client {
            let _ = client.deal_new_stream(session).await;
            None
        } else {
            let _ = self.deal_proxy(session).await;
            None
        }
    }

    async fn ready_init(&mut self) -> io::Result<()> {
        match self.config.try_connect_center_client().await {
            Ok((client_config, center_client)) => {
                self.client_config = client_config;
                self.center_client = center_client;
            }
            Err(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "connect to center failed",
                ));
            }
        }
        Ok(())
    }
}
