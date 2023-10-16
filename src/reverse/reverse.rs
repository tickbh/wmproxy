use std::{net::SocketAddr, sync::Arc};

use tokio::{io::{AsyncRead, AsyncWrite}, sync::Mutex};
use webparse::{Request, Response};
use wenmeng::{Server, RecvStream, ProtResult, ProtError};

use crate::ProxyResult;

use super::ReverseOption;


pub struct Reverse {
    
}

impl Reverse {
    
    async fn inner_operate(
        mut req: Request<RecvStream>
    ) -> ProtResult<Response<RecvStream>> {
        println!("receiver req = {:?}", req.url());
        let data = req.extensions_mut().remove::<Arc<Mutex<ReverseOption>>>();
        if data.is_none() {
            return Err(ProtError::Extension("unknow data"));
        }
        let data = data.unwrap();
        let mut value = data.lock().await;
        // value.file_server.deal_request(req).await
        Err(ProtError::Extension("unknow data"))
    }

    async fn operate(
        req: Request<RecvStream>
    ) -> ProtResult<Response<RecvStream>> {
        let mut value = Self::inner_operate(req).await?;
        value.headers_mut().insert("server", "wmproxy");
        Ok(value)
    }

    pub async fn process<T>(
        inbound: T, 
        addr: SocketAddr,
        option: &mut ReverseOption
    ) -> ProxyResult<()>     where
    T: AsyncRead + AsyncWrite + Unpin {
        let mut server = Server::new(inbound, Some(addr), option.clone());
        let _ret = server.incoming(Self::operate).await;
        if _ret.is_err() {
            println!("ret = {:?}", _ret);
        }
        Ok(())
    }
}