use std::{io, net::IpAddr, sync::Arc};
use async_trait::async_trait;
use rustls::ClientConfig;
use webparse::BinaryMut;

use crate::{core::{AppTrait, Listeners, Service, ShutdownWatch, Stream}, error::ProxyTypeResult, CenterClient, ConfigHeader, Flag, ProxyConfig, ProxyError, ProxyHttp, ProxySocks5};


pub struct ProxyApp {
    flag: Flag,
    username: Option<String>,
    password: Option<String>,
    udp_bind: Option<IpAddr>,
    headers: Option<Vec<ConfigHeader>>,
    config: Option<ProxyConfig>,
    client_config: Option<Arc<ClientConfig>>,
    center_client: Option<CenterClient>,
}

impl ProxyApp {
    pub fn new(
        flag: Flag,
        username: Option<String>,
        password: Option<String>,
        udp_bind: Option<IpAddr>,
        headers: Option<Vec<ConfigHeader>>,
    ) -> Self {
        Self {
            flag,
            username,
            password,
            udp_bind,
            headers,
            config: None,
            client_config: None,
            center_client: None,
        }
    }

    pub fn set_config(&mut self, config: ProxyConfig) {
        self.config = Some(config);
    }

    pub async fn deal_proxy(
        &self,
        inbound: Stream,
    ) -> ProxyTypeResult<(), Stream>
    {
        let (read_buf, inbound) = match self.process_http(inbound).await {
            Ok(()) => {
                return Ok(());
            }
            Err(ProxyError::Continue(buf)) => buf,
            Err(err) => return Err(err),
        };

        let _read_buf =
            match self.process_socks5(inbound, read_buf).await
            {
                Ok(()) => return Ok(()),
                Err(ProxyError::Continue(buf)) => buf,
                Err(err) => {
                    log::info!("socks5代理发生错误：{:?}", err);
                    return Err(err);
                }
            };
        Ok(())
    }

    
    async fn process_http(
        &self,
        inbound: Stream,
    ) -> ProxyTypeResult<(), Stream>
    {
        if self.flag.contains(Flag::HTTP) || self.flag.contains(Flag::HTTPS) {
            ProxyHttp::process(&self.username, &self.password, self.headers.clone(), inbound).await
        } else {
            Err(ProxyError::Continue((None, inbound)))
        }
    }

    async fn process_socks5(
        &self,
        inbound: Stream,
        buffer: Option<BinaryMut>,
    ) -> ProxyTypeResult<(), Stream>
    {
        if self.flag.contains(Flag::SOCKS5) {
            let mut sock = ProxySocks5::new(self.username.clone(), self.password.clone(), self.udp_bind);
            sock.process(inbound, buffer).await
        } else {
            Err(ProxyError::Continue((buffer, inbound)))
        }
    }

    pub fn build_services(self, listeners: Listeners) -> Service<Self> {
        Service::with_listeners("proxy_app".to_string(), listeners, self)
    }
}

#[async_trait]
impl AppTrait for ProxyApp {
    async fn process_new(
        self: &Arc<Self>,
        session: Stream,
        _shutdown: &ShutdownWatch,
    ) -> Option<Stream> {
        println!("aaaaaaaaaaaaaaa");
        
        if let Some(client) = &self.center_client {
            let _ = client.deal_new_stream(session).await;
            None
        } else {
            let _ = self.deal_proxy(session).await;
            println!("bbbbbbbbbbbbbbbbbb"); 
            None
        }

    }

    async fn ready_init(&mut self) -> io::Result<()> {
        if let Some(config) = &self.config {
            match config.try_connect_center_client().await {
                Ok((client_config, center_client)) => {
                    self.client_config = client_config;
                    self.center_client = center_client;
                },
                Err(_) => {
                    return Err(io::Error::new(io::ErrorKind::Other, "connect to center failed"));
                },
            }
        }
        Ok(())
    }
}