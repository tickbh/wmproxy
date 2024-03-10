use std::{net::IpAddr, sync::Arc};
use async_trait::async_trait;
use webparse::BinaryMut;

use crate::{core::{AppTrait, ShutdownWatch, Stream}, error::ProxyTypeResult, ConfigHeader, Flag, ProxyError, ProxyHttp, ProxySocks5};


pub struct ClientApp {
    flag: Flag,
    username: Option<String>,
    password: Option<String>,
    udp_bind: Option<IpAddr>,
    headers: Option<Vec<ConfigHeader>>,
}

impl ClientApp {
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
        }
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
}

#[async_trait]
impl AppTrait for ClientApp {
    async fn process_new(
        self: &Arc<Self>,
        session: Stream,
        shutdown: &ShutdownWatch,
    ) -> Option<Stream> {
        self.deal_proxy(session).await;
        todo!()
    }
}