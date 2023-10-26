

use std::sync::Arc;

use tokio::{sync::{mpsc::{Sender, channel, Receiver}, Mutex}, net::TcpListener};
use webparse::{Request, Response, HeaderName};
use wenmeng::{Server, RecvStream, ProtResult, ProtError};

use crate::{ConfigOption, Proxy, ProxyResult, control::server, reverse::HttpConfig};

/// 控制端，可以对配置进行热更新
pub struct ControlServer {
    /// 控制端当前的配置文件，如果部分修改将直接修改数据进行重启
    option: ConfigOption,
    /// 通知服务进行关闭的Sender，服务相关如果收到该消息则停止Accept
    server_sender_close: Option<Sender<()>>,
    /// 通知中心服务的Sender，每个服务拥有一个该Sender，可反向通知中控关闭
    control_sender_close: Sender<()>,
    /// 通知中心服务的Receiver，收到一次则将当前的引用计数-1，如果为0则表示需要关闭服务器
    control_receiver_close: Option<Receiver<()>>,
    /// 服务的引用计数
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
        // 每次启动的时候将让控制计数+1
        self.count += 1;
        tokio::spawn(async move {
            let mut proxy = Proxy::new(option);
            // 将上一个进程的关闭权限交由下一个服务，只有等下一个服务准备完毕的时候才能关闭上一个服务
            if let Err(e) = proxy.start_serve(receiver_no_listen, sender_close).await {
                log::info!("处理失败服务进程失败: {:?}", e);
            }
            // 每次退出的时候将让控制计数-1，减到0则退出
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
            // 将重新启动服务器
            let _ = value.do_restart_serve().await;
            return Ok(Response::text()
            .body("重新加载配置成功")
            .unwrap()
            .into_type());
        }

        if req.path() == "/stop" {
            // 通知控制端关闭，控制端阻塞主线程，如果控制端退出后进程退出
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
