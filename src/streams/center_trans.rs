// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//
// Author: tickbh
// -----
// Created Date: 2023/09/25 10:08:56

use std::{io, sync::Arc};

use tokio::{
    io::{AsyncRead, AsyncWrite},
};

use tokio_rustls::{TlsConnector};


use wenmeng::MaybeHttpsStream;

use crate::{
    HealthCheck, ProxyResult,
};

/// 中心服务端
/// 接受中心客户端的连接，并且将信息处理或者转发
pub struct CenterTrans {
    server: String,
    domain: Option<String>,
    tls_client: Option<Arc<rustls::ClientConfig>>,
}

impl CenterTrans {
    pub fn new(
        server: String,
        domain: Option<String>,
        tls_client: Option<Arc<rustls::ClientConfig>>,
    ) -> Self {
        Self {
            server,
            domain,
            tls_client,
        }
    }

    pub async fn serve<T>(&mut self, mut stream: T) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let mut server = if self.tls_client.is_some() {
            let connector = TlsConnector::from(self.tls_client.clone().unwrap());
            let stream = HealthCheck::connect(&self.server).await?;
            // 这里的域名只为认证设置
            let domain = rustls::ServerName::try_from(
                &*self
                    .domain
                    .clone()
                    .unwrap_or("soft.wm-proxy.com".to_string()),
            )
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;

            let outbound = connector.connect(domain, stream).await?;
            MaybeHttpsStream::Https(outbound)
        } else {
            let outbound = HealthCheck::connect(&self.server).await?;
            MaybeHttpsStream::Http(outbound)
        };

        tokio::spawn(async move {
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut server).await;
        });
        Ok(())
    }
}
