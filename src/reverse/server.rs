use std::{net::SocketAddr, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::{sync::Mutex, io::{AsyncRead, AsyncWrite}};
use webparse::{Request, Response, http::response};
use wenmeng::{RecvStream, ProtResult, ProtError, Server};

use crate::{reverse::ReverseOption, ProxyResult};

use super::LocationConfig;

fn default_location() -> Vec<LocationConfig> {
    vec![]
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    #[serde(default = "default_location")]
    pub location: Vec<LocationConfig>,
}

impl ServerConfig {

    async fn inner_operate(
        mut req: Request<RecvStream>
    ) -> ProtResult<Response<RecvStream>> {
        println!("receiver req = {:?}", req.url());
        let data = req.extensions_mut().remove::<Arc<Mutex<ServerConfig>>>();
        if data.is_none() {
            return Err(ProtError::Extension("unknow data"));
        }
        let data = data.unwrap();
        let mut value = data.lock().await;
        let path = req.path().clone();
        for l in &mut value.location {
            if l.is_match_rule(&path) {
                return l.deal_request(req).await;
            }
        }
        return Ok(Response::builder().status(503).body("unknow location").unwrap().into_type());
    }

    async fn operate(
        req: Request<RecvStream>
    ) -> ProtResult<Response<RecvStream>> {
        let mut value = Self::inner_operate(req).await?;
        value.headers_mut().insert("server", "wmproxy");
        Ok(value)
    }

    pub async fn process<T>(
        &mut self,
        inbound: T, 
        addr: SocketAddr,
    ) -> ProxyResult<()>     where
    T: AsyncRead + AsyncWrite + Unpin + std::marker::Send + 'static {
        println!("xxxxxxxxxxxxxxxxxxxx");
        let option = self.clone();
        tokio::spawn(async move {
            let mut server = Server::new(inbound, Some(addr), option);
            let _ret = server.incoming(Self::operate).await;
            if _ret.is_err() {
                println!("ret = {:?}", _ret);
            };
            
        });
        Ok(())
    }
}