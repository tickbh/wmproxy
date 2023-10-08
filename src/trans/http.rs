use std::{
    io::{self, Read},
    sync::Arc,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    sync::{
        mpsc::{channel, Sender, Receiver},
        Mutex, RwLock,
    },
};
use webparse::{
    http2::frame::Frame, Binary, BinaryMut, Buf, BufMut, HttpError, Request, Response, Serialize,
    WebError,
};
use wenmeng::{Client, ProtResult, RecvStream, Server, ProtError};

use crate::{MappingConfig, ProtCreate, ProtFrame, ProxyError, TransStream, VirtualStream};

pub struct TransHttp {
    sender: Sender<ProtFrame>,
    sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    sock_map: u32,
    mappings: Arc<RwLock<Vec<MappingConfig>>>,
}

struct HttpOper {
    pub receiver: Receiver<Response<RecvStream>>,
    pub sender: Sender<Request<RecvStream>>,
    pub virtual_sender: Option<Sender<ProtFrame>>,
    pub sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    pub sock_map: u32,
    pub mappings: Arc<RwLock<Vec<MappingConfig>>>,
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

    async fn err_server_status<T>(mut inbound: T, status: u16) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut res = webparse::Response::builder().status(status).body(())?;
        inbound.write_all(&res.httpdata()?).await?;
        Ok(())
    }

    async fn not_match_err_status<T>(mut inbound: T, body: String) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut res = webparse::Response::builder().status(503).body(body)?;
        inbound.write_all(&res.httpdata()?).await?;
        Ok(())
    }

    // pub async fn process<T>(self, mut inbound: T) -> Result<(), ProxyError<T>>
    // where
    //     T: AsyncRead + AsyncWrite + Unpin,
    // {
    //     let mut request;
    //     let host_name;
    //     let mut buffer = BinaryMut::new();
    //     loop {
    //         let size = {
    //             let mut buf = ReadBuf::uninit(buffer.chunk_mut());
    //             inbound.read_buf(&mut buf).await?;
    //             buf.filled().len()
    //         };

    //         if size == 0 {
    //             return Err(ProxyError::Extension("empty"));
    //         }
    //         unsafe {
    //             buffer.advance_mut(size);
    //         }
    //         request = webparse::Request::new();
    //         // 通过该方法解析标头是否合法, 若是partial(部分)则继续读数据
    //         // 若解析失败, 则表示非http协议能处理, 则抛出错误
    //         // 此处clone为浅拷贝，不确定是否一定能解析成功，不能影响偏移
    //         match request.parse_buffer(&mut buffer.clone()) {
    //             Ok(_) => match request.get_host() {
    //                 Some(host) => {
    //                     host_name = host;
    //                     break;
    //                 }
    //                 None => {
    //                     if !request.is_partial() {
    //                         Self::err_server_status(inbound, 503).await?;
    //                         return Err(ProxyError::UnknownHost);
    //                     }
    //                 }
    //             },
    //             // 数据不完整，还未解析完，等待传输
    //             Err(WebError::Http(HttpError::Partial)) => {
    //                 continue;
    //             }
    //             Err(e) => {
    //                 Self::not_match_err_status(inbound, "not found".to_string()).await?;
    //                 return Err(ProxyError::from(e));
    //             }
    //         }
    //     }

    //     // 取得相关的host数据，对内网的映射端做匹配，如果未匹配到返回错误，表示不支持
    //     {
    //         let mut is_find = false;
    //         let read = self.mappings.read().await;
    //         for v in &*read {
    //             if v.domain == host_name {
    //                 is_find = true;
    //             }
    //         }
    //         if !is_find {
    //             Self::not_match_err_status(inbound, "no found".to_string()).await?;
    //             return Ok(());
    //         }
    //     }

    //     // 有新的内网映射消息到达，通知客户端建立对内网指向的连接进行双向绑定，后续做正规的http服务以支持拓展
    //     let create = ProtCreate::new(self.sock_map, Some(host_name));
    //     let (stream_sender, stream_receiver) = channel::<ProtFrame>(10);
    //     let _ = self.sender_work.send((create, stream_sender)).await;

    //     let mut trans = TransStream::new(inbound, self.sock_map, self.sender, stream_receiver);
    //     trans.reader_mut().put_slice(buffer.chunk());
    //     trans.copy_wait().await?;
    //     // let _ = copy_bidirectional(&mut inbound, &mut outbound).await?;
    //     Ok(())
    // }

    //, client: &mut Client<VirtualStream>
    async fn operate(
        mut req: Request<RecvStream>,
        data: Arc<Mutex<HttpOper>>,
    ) -> ProtResult<Option<Response<RecvStream>>> {
        println!("receiver req = {:?}", req);
        let mut value = data.lock().await;
        let sender = value.virtual_sender.take();
        if sender.is_some() {
            let host_name = req.get_host().unwrap_or(String::new());
            // 取得相关的host数据，对内网的映射端做匹配，如果未匹配到返回错误，表示不支持
            {
                let mut is_find = false;
                let read = value.mappings.read().await;
                for v in &*read {
                    if v.domain == host_name {
                        is_find = true;
                    }
                }
                if !is_find {
                    return Ok(Some(Response::builder().status(404).body("not found").ok().unwrap().into_type()));
                }
            }

            let create = ProtCreate::new(value.sock_map, Some(req.get_host().unwrap_or(String::new())));
            let _ = value.sender_work.send((create, sender.unwrap())).await;
        }

        value.sender.send(req).await?;
        let res = value.receiver.recv().await;
        if res.is_some() {
            return Ok(res);
        } else {
            return Ok(Some(Response::builder().status(503).body("cant trans").ok().unwrap().into_type()));
        }
    }

    pub async fn process<T>(self, inbound: T) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let build = Client::builder();

        // let create = ProtCreate::new(self.sock_map, Some(host_name));
        // let (stream_sender, stream_receiver) = channel::<ProtFrame>(10);
        // let _ = self.sender_work.send((create, stream_sender)).await;

        let (virtual_sender, virtual_receiver) = channel::<ProtFrame>(10);
        let stream = VirtualStream::new(self.sock_map, self.sender.clone(), virtual_receiver);
        let mut client = Client::new(build.value().ok().unwrap(), stream);
        let (receiver, sender) = client.split().unwrap();
        let oper = HttpOper {
            receiver,
            sender,
            sender_work: self.sender_work.clone(),
            virtual_sender: Some(virtual_sender),
            sock_map: self.sock_map,
            mappings: self.mappings.clone(),
        };
        let mut server = Server::new(inbound, oper);
        tokio::spawn( async move {
            let _ = client.wait_operate().await;
        });
        println!("aaaaaaaaaaaaa");
        let _ret = server.incoming(Self::operate).await;
        if _ret.is_err() {
            println!("ret = {:?}", _ret);
        }
        Ok(())
    }
}
