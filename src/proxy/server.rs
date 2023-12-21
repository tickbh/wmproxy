use std::net::IpAddr;

use tokio::io::{AsyncRead, AsyncWrite};
use webparse::BinaryMut;

use crate::{Flag, MappingConfig, error::ProxyTypeResult, ProxyError, ProxyHttp, ProxySocks5, ConfigHeader};

/// 代理服务器类, 提供代理服务
pub struct ProxyServer {
    flag: Flag,
    username: Option<String>,
    password: Option<String>,
    udp_bind: Option<IpAddr>,
    headers: Option<Vec<ConfigHeader>>,
}

impl ProxyServer {
    pub fn new(
        flag: Flag,
        username: Option<String>,
        password: Option<String>,
        udp_bind: Option<IpAddr>,
        headers: Option<Vec<ConfigHeader>>,
    ) -> Self {
        ProxyServer {
            flag,
            username,
            password,
            udp_bind,
            headers,
        }
    }
    
    pub async fn deal_proxy<T>(
        mut self,
        inbound: T,
    ) -> ProxyTypeResult<(), T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
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

    
    async fn process_http<T>(
        &mut self,
        inbound: T,
    ) -> ProxyTypeResult<(), T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if self.flag.contains(Flag::HTTP) || self.flag.contains(Flag::HTTPS) {
            ProxyHttp::process(&self.username, &self.password, self.headers.take(), inbound).await
        } else {
            Err(ProxyError::Continue((None, inbound)))
        }
    }

    async fn process_socks5<T>(
        self,
        inbound: T,
        buffer: Option<BinaryMut>,
    ) -> ProxyTypeResult<(), T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        if self.flag.contains(Flag::SOCKS5) {
            let mut sock = ProxySocks5::new(self.username, self.password, self.udp_bind);
            sock.process(inbound, buffer).await
        } else {
            Err(ProxyError::Continue((buffer, inbound)))
        }
    }
}
