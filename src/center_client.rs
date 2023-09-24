use std::{
    io,
    net::SocketAddr,
    sync::{mpsc::Receiver, Arc},
};
use tokio::net::TcpStream;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::Sender,
};
use tokio_rustls::{TlsConnector, TlsStream};

use crate::{prot::ProtFrame, ProxyResult};

// pub struct Builder {
//     server_addr: SocketAddr,
//     tls_client: Option<Arc<rustls::ClientConfig>>,
//     domain: Option<String>,
// }

// impl Builder {
//     pub fn new(
//         server_addr: SocketAddr,
//         tls_client: Option<Arc<rustls::ClientConfig>>,
//         domain: Option<String>,
//     ) -> Builder {
//         Self {
//             tls_client,
//             domain,
//             server_addr,
//         }
//     }

//     pub async fn connect_tls(self) -> ProxyResult<Server<tokio_rustls::client::TlsStream<TcpStream>>> {
//         let connector = TlsConnector::from(self.tls_client.clone().unwrap());
//         let stream = TcpStream::connect(&self.server_addr).await?;
//         // 这里的域名只为认证设置
//         let domain =
//             rustls::ServerName::try_from(&*self.domain.clone().unwrap_or("soft.wm-proxy.com".to_string()))
//                 .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;

//         let stream = connector.connect(domain, stream).await?;
//         Ok(Server::new(stream, self.server_addr, self.tls_client, self.domain))
//     }

//     pub async fn connect(self) -> ProxyResult<Server<TcpStream>> {
//         let stream = TcpStream::connect(self.server_addr).await?;
//         Ok(Server::new(stream, self.server_addr, None, None))
//     }
// }

pub struct CenterClient {
    tls_client: Option<Arc<rustls::ClientConfig>>,
    domain: Option<String>,
    server_addr: SocketAddr,
    sender: Option<Sender<ProtFrame>>,
    receiver: Option<Receiver<ProtFrame>>,
}

impl CenterClient {
    pub fn new(
        server_addr: SocketAddr,
        tls_client: Option<Arc<rustls::ClientConfig>>,
        domain: Option<String>,
    ) -> Self {
        Self {
            tls_client,
            domain,
            server_addr,
            sender: None,
            receiver: None,
        }
    }

    pub async fn transfer<T>()
    where T : AsyncRead + AsyncWrite + Unpin {

    }

    pub async fn serve(&mut self) {
        let tls_client = self.tls_client.clone();
        let server = self.server_addr.clone();
        let domain = self.domain.clone();

        tokio::spawn(async move {
            // if self.tls_client.is_some() {
            //     println!("connect by tls");
            //     let connector = TlsConnector::from(tls_client.unwrap());
            //     let stream = TcpStream::connect(&server).await?;
            //     // 这里的域名只为认证设置
            //     let domain =
            //         rustls::ServerName::try_from(&*domain.unwrap_or("soft.wm-proxy.com".to_string()))
            //             .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid dnsname"))?;
    
            //     if let Ok(mut outbound) = connector.connect(domain, stream).await {
            //         // connect 之后的流跟正常内容一样读写, 在内部实现了自动加解密
            //         let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await?;
            //     } else {
            //         // TODO 返回对应协议的错误
            //         // let _ = Self::deal_proxy(inbound, flag, None, None, None).await;
            //     }
            // } else {
            //     println!("connect by normal");
            //     if let Ok(mut outbound) = TcpStream::connect(server).await {
            //         let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await?;
            //     } else {
            //         // TODO 返回对应协议的错误
            //         // let _ = Self::deal_proxy(inbound, flag, None, None, None).await;
            //     }
            // }
        });
    }
}
