

use std::sync::Arc;

use tokio::{sync::{mpsc::{Sender, channel, Receiver}, Mutex}, net::TcpListener};
use webparse::{Request, Response, HeaderName};
use wenmeng::{Server, RecvStream, ProtResult, ProtError};

use crate::{ConfigOption, Proxy, ProxyResult, control::server, reverse::HttpConfig};

pub struct ControlServer {
    option: ConfigOption,
    server_sender_close: Option<Sender<()>>,
    control_sender_close: Sender<()>,
    control_receiver_close: Option<Receiver<()>>,
    count: i32,
}

impl ControlServer {
    pub fn new(option: ConfigOption) -> Self {
        let (sender, receiver) = channel::<()>(1);
        Self {
            option,
            server_sender_close: None,
            control_sender_close: sender,
            control_receiver_close: Some(receiver),
            count: 0,
        }
    }

    pub async fn start_serve(mut self) -> ProxyResult<()> {
        let option = self.option.clone();
        self.inner_start_server(option).await?;
        Self::start_control(Arc::new(Mutex::new(self))).await?;
        Ok(())
    }

    pub async fn do_restart_serve(&mut self) -> ProxyResult<()> {
        let option = ConfigOption::parse_env()?;
        self.inner_start_server(option).await?;
        Ok(())
    }

    async fn inner_start_server(&mut self, option: ConfigOption) -> ProxyResult<()>  {
        let sender = self.control_sender_close.clone();
        let (sender_no_listen, receiver_no_listen) = channel::<()>(1);
        let sender_close = self.server_sender_close.take();
        self.count += 1;
        tokio::spawn(async move {
            let mut proxy = Proxy::new(option);
            if let Err(e) = proxy.start_serve(receiver_no_listen, sender_close).await {
                log::info!("处理失败服务进程失败: {:?}", e);
            }
            let _ = sender.send(()).await;
        });
        self.server_sender_close = Some(sender_no_listen);
        Ok(())
    }

    async fn inner_operate(mut req: Request<RecvStream>) -> ProtResult<Response<RecvStream>> {
        let data = req.extensions_mut().remove::<Arc<Mutex<ControlServer>>>();
        if data.is_none() {
            return Err(ProtError::Extension("unknow data"));
        }
        let data = data.unwrap();
        let mut value = data.lock().await;
        if req.path() == "/reload" {
            let _ = value.do_restart_serve().await;
            return Ok(Response::text()
            .body("重新加载配置成功")
            .unwrap()
            .into_type());
        }

        if req.path() == "/stop" {
            if let Some(sender) = &value.server_sender_close {
                let _ = sender.send(()).await;
            }
            return Ok(Response::text()
            .body("关闭进程成功")
            .unwrap()
            .into_type());
        }

        return Ok(Response::status503()
            .body("服务器内部无服务")
            .unwrap()
            .into_type());
    }

    async fn operate(req: Request<RecvStream>) -> ProtResult<Response<RecvStream>> {
        // body的内容可能重新解密又再重新再加过密, 后续可考虑直接做数据
        let mut value = Self::inner_operate(req).await?;
        value.headers_mut().insert("server", "wmproxy");
        Ok(value)
    }

    async fn receiver_await(receiver: &mut Option<Receiver<()>>) -> Option<()> {
        if receiver.is_some() {
            receiver.as_mut().unwrap().recv().await
        } else {
            let pend = std::future::pending();
            let () = pend.await;
            None
        }
    }
    
    pub async fn start_control(control: Arc<Mutex<ControlServer>>) -> ProxyResult<()> {
        let listener = {
            let value = &control.lock().await.option;
            TcpListener::bind(format!("127.0.0.1:{}", value.control)).await?
        };

        loop {
            let mut receiver = {
                let value = &mut control.lock().await;
                value.control_receiver_close.take()
            };
            
            tokio::select! {
                Ok((conn, addr)) = listener.accept() => {
                    let cc = control.clone();
                    tokio::spawn(async move {
                        let mut server = Server::new_data(conn, Some(addr), cc);
                        if let Err(e) = server.incoming(Self::operate).await {
                            log::info!("反向代理：处理信息时发生错误：{:?}", e);
                        }
                    });
                    let value = &mut control.lock().await;
                    value.control_receiver_close = receiver;
                }
                _ = Self::receiver_await(&mut receiver) => {
                    // log::info!("反向代理：控制端收到关闭信号");
                    let value = &mut control.lock().await;
                    value.count -= 1;
                    log::info!("反向代理：控制端收到关闭信号，当前:{}", value.count);
                    if value.count <= 0 {
                        break;
                    }
                    value.control_receiver_close = receiver;
                }
            }
        }
        Ok(())
    }
}
