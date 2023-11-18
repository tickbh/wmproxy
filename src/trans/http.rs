use std::{
    io::{Read},
    sync::Arc, fmt::Debug, net::SocketAddr,
};

use async_trait::async_trait;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite},
    sync::{
        mpsc::{channel, Sender, Receiver},
        Mutex, RwLock,
    },
};
use webparse::{
    Request, Response,
};
use wenmeng::{Client, ProtResult, RecvStream, Server, HeaderHelper, ProtError, OperateTrait, RecvRequest, RecvResponse};

use crate::{MappingConfig, ProtCreate, ProtFrame, ProxyError, VirtualStream};

struct Operate;
#[async_trait]
impl OperateTrait for Operate {
    async fn operate(&self, req: &mut RecvRequest) -> ProtResult<RecvResponse> {
        TransHttp::operate(req).await
    }
}

pub struct TransHttp {
    sender: Sender<ProtFrame>,
    sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    sock_map: u32,
    mappings: Arc<RwLock<Vec<MappingConfig>>>,
}

struct HttpOper {
    pub receiver: Receiver<ProtResult<Response<RecvStream>>>,
    pub sender: Sender<Request<RecvStream>>,
    pub virtual_sender: Option<Sender<ProtFrame>>,
    pub sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    pub sock_map: u32,
    pub mappings: Arc<RwLock<Vec<MappingConfig>>>,
    pub http_map: Option<MappingConfig>,
}


impl TransHttp {
    pub fn new(
        sender: Sender<ProtFrame>,
        sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
        sock_map: u32,
        mappings: Arc<RwLock<Vec<MappingConfig>>>,
    ) -> Self {
        Self {
            sender,
            sender_work,
            sock_map,
            mappings,
        }
    }

    async fn operate(
        req: &mut Request<RecvStream>
    ) -> ProtResult<Response<RecvStream>> {
        let mut value = Self::inner_operate(req).await?;
        value.headers_mut().insert("server", "wmproxy");
        Ok(value)
    }

    async fn inner_operate(
        req: &mut Request<RecvStream>
    ) -> ProtResult<Response<RecvStream>> {
        let data = req.extensions_mut().remove::<Arc<Mutex<HttpOper>>>();
        if data.is_none() {
            return Err(ProtError::Extension("unknow data"));
        }
        let data = data.unwrap();
        let mut value = data.lock().await;
        let sender = value.virtual_sender.take();
        // 传在该参数则为第一次, 第一次的时候发送Create创建绑定连接
        if sender.is_some() {
            let host_name = req.get_host().unwrap_or(String::new());
            // 取得相关的host数据，对内网的映射端做匹配，如果未匹配到返回错误，表示不支持
            {
                let mut config = None;
                let mut is_find = false;
                {
                    let read = value.mappings.read().await;
                    for v in &*read {
                        if v.domain == host_name {
                            is_find = true;
                            config = Some(v.clone());
                        }
                    }
                }
                if !is_find {
                    return Ok(Response::builder().status(404).body("not found").ok().unwrap().into_type());
                }
                value.http_map = config;
            }

            let create = ProtCreate::new(value.sock_map, Some(req.get_host().unwrap_or(String::new())));
            let _ = value.sender_work.send((create, sender.unwrap())).await;
        }

        if let Some(config) = &value.http_map {
            // 复写Request的头文件信息
            HeaderHelper::rewrite_request(req, &config.headers);
        }

        // 将请求发送出去
        value.sender.send(req.replace_clone(RecvStream::empty())).await?;
        // 等待返回数据的到来
        let res = value.receiver.recv().await;
        if res.is_some() && res.as_ref().unwrap().is_ok() {
            let mut res = res.unwrap().unwrap();
            if let Some(config) = &value.http_map {
                // 复写Response的头文件信息
                HeaderHelper::rewrite_response(&mut res, &config.headers);
            }
            return Ok(res);
        } else {
            return Ok(Response::builder().status(503).body("cant trans").ok().unwrap().into_type());
        }
    }

    pub async fn process<T>(self, inbound: T, addr: SocketAddr) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin + Debug,
    {
        log::trace!("内网穿透处理HTTP {:?}", addr);
        let build = Client::builder();
        let (virtual_sender, virtual_receiver) = channel::<ProtFrame>(10);
        let stream = VirtualStream::new(self.sock_map, self.sender.clone(), virtual_receiver);
        let mut client = Client::new(build.value(), stream);
        let (receiver, sender) = client.split().unwrap();
        let oper = HttpOper {
            receiver,
            sender,
            sender_work: self.sender_work.clone(),
            virtual_sender: Some(virtual_sender),
            sock_map: self.sock_map,
            mappings: self.mappings.clone(),
            http_map: None,
        };
        let mut server = Server::new_data(inbound, Some(addr), Arc::new(Mutex::new(oper)) );
        tokio::spawn( async move {
            let _ = client.wait_operate().await;
        });
        if let Err(e) = server.incoming(Operate).await {
            log::info!("处理内网穿透时发生错误：{:?}", e);
        };
        Ok(())
    }
}
