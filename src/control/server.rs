

use std::sync::Arc;

use tokio::{sync::{mpsc::{Sender, channel, Receiver}, Mutex}, net::TcpListener};
use webparse::{Request, Response};
use wenmeng::{Server, RecvStream, ProtResult, ProtError};

use crate::{ConfigOption, Proxy, ProxyResult, control::server, reverse::HttpConfig};

pub struct ControlServer {
    option: ConfigOption,
    sender_close: Option<Sender<()>>,
    receiver_close: Option<Receiver<()>>,
}

impl ControlServer {
    pub fn new(option: ConfigOption) -> Self {
        Self {
            option,
            sender_close: None,
            receiver_close: None,
        }
    }

    pub async fn start_server(mut self) -> ProxyResult<()> {
        let option = self.option.clone();
        let (sender, mut receiver) = channel::<()>(1);
        tokio::spawn(async move {
            let mut proxy = Proxy::new(option);
            let _ = proxy.start_serve().await;
            let _ = sender.send(()).await;
        });
        self.receiver_close = Some(receiver);
        Self::start_control(Arc::new(Mutex::new(self))).await?;
        Ok(())
    }


    async fn inner_operate(mut req: Request<RecvStream>) -> ProtResult<Response<RecvStream>> {
        // let data = req.extensions_mut().remove::<Arc<Mutex<ControlServer>>>();
        // if data.is_none() {
        //     return Err(ProtError::Extension("unknow data"));
        // }
        // let data = data.unwrap();
        // let value = data.lock().await;
        // let server_len = value.server.len();
        // let host = req.get_host().unwrap_or(String::new());
        // // 不管有没有匹配, 都执行最后一个
        // for (index, s) in value.server.iter().enumerate() {
        //     if s.server_name == host || host.is_empty() || index == server_len - 1 {
        //         let path = req.path().clone();
        //         for l in s.location.iter() {
        //             if l.is_match_rule(&path, req.method()) {
        //                 return l.deal_request(req).await;
        //             }
        //         }
        //         return Ok(Response::builder()
        //             .status(503)
        //             .body("unknow location to deal")
        //             .unwrap()
        //             .into_type());
        //     }
        // }
        return Ok(Response::builder()
            .status(503)
            .body("unknow location")
            .unwrap()
            .into_type());
    }

    async fn operate(req: Request<RecvStream>) -> ProtResult<Response<RecvStream>> {
        // body的内容可能重新解密又再重新再加过密, 后续可考虑直接做数据
        let mut value = Self::inner_operate(req).await?;
        value.headers_mut().insert("server", "wmproxy");
        Ok(value)
    }
    
    pub async fn start_control(control: Arc<Mutex<ControlServer>>) -> ProxyResult<()> {
        let value = &control.lock().await.option;
        let listener = TcpListener::bind(format!("127.0.0.1:{}", value.control)).await?;

        tokio::select! {
            Ok((conn, addr)) = listener.accept() => {
                let cc = control.clone();
                tokio::spawn(async move {
                    let mut server = Server::new_data(conn, Some(addr), cc);
                    if let Err(e) = server.incoming(Self::operate).await {
                        log::info!("反向代理：处理信息时发生错误：{:?}", e);
                    }
                });
            }
        }
        Ok(())
    }
}
