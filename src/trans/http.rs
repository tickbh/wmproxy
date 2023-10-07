use std::{sync::Arc, io::{self, Read}};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    sync::{mpsc::{Sender, channel}, RwLock, Mutex},
};
use webparse::{BinaryMut, Buf, BufMut, HttpError, WebError, Response, Request, Serialize, Binary, http2::frame::Frame};
use wenmeng::{RecvStream, ProtResult, Server, Client};


use crate::{ProtFrame, TransStream, ProxyError, ProtCreate, MappingConfig, VirtualStream};

pub struct TransHttp {
    sender: Sender<ProtFrame>,
    sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    sock_map: u32,
    mappings: Arc<RwLock<Vec<MappingConfig>>>,
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

    pub async fn process<T>(self, mut inbound: T) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut request;
        let host_name;
        let mut buffer = BinaryMut::new();
        loop {
            let size = {
                let mut buf = ReadBuf::uninit(buffer.chunk_mut());
                inbound.read_buf(&mut buf).await?;
                buf.filled().len()
            };

            if size == 0 {
                return Err(ProxyError::Extension("empty"));
            }
            unsafe {
                buffer.advance_mut(size);
            }
            request = webparse::Request::new();
            // 通过该方法解析标头是否合法, 若是partial(部分)则继续读数据
            // 若解析失败, 则表示非http协议能处理, 则抛出错误
            // 此处clone为浅拷贝，不确定是否一定能解析成功，不能影响偏移
            match request.parse_buffer(&mut buffer.clone()) {
                Ok(_) => match request.get_host() {
                    Some(host) => {
                        host_name = host;
                        break;
                    }
                    None => {
                        if !request.is_partial() {
                            Self::err_server_status(inbound, 503).await?;
                            return Err(ProxyError::UnknownHost);
                        }
                    }
                },
                // 数据不完整，还未解析完，等待传输
                Err(WebError::Http(HttpError::Partial)) => {
                    continue;
                }
                Err(e) => {
                    Self::not_match_err_status(inbound, "not found".to_string()).await?;
                    return Err(ProxyError::from(e));
                }
            }
        }

        // 取得相关的host数据，对内网的映射端做匹配，如果未匹配到返回错误，表示不支持
        {
            let mut is_find = false;
            let read = self.mappings.read().await;
            for v in &*read {
                if v.domain == host_name {
                    is_find = true;
                }
            }
            if !is_find {
                Self::not_match_err_status(inbound, "no found".to_string()).await?;
                return Ok(());
            }
        }

        // 有新的内网映射消息到达，通知客户端建立对内网指向的连接进行双向绑定，后续做正规的http服务以支持拓展
        let create = ProtCreate::new(self.sock_map, Some(host_name));
        let (stream_sender, stream_receiver) = channel::<ProtFrame>(10);
        let _ = self.sender_work.send((create, stream_sender)).await;
        
        let mut trans = TransStream::new(inbound, self.sock_map, self.sender, stream_receiver);
        trans.reader_mut().put_slice(buffer.chunk());
        trans.copy_wait().await?;
        // let _ = copy_bidirectional(&mut inbound, &mut outbound).await?;
        Ok(())
    }

    //, client: &mut Client<VirtualStream>
    async fn operate(mut req: Request<RecvStream>, data: Arc<Mutex<(Client<VirtualStream>, Sender<ProtFrame>)>>) -> ProtResult<Option<Response<RecvStream>>> { //, client: Client<VirtualStream>, sender: Sender<ProtFrame>
        let mut builder = Response::builder().version(req.version().clone());
        let body = match &*req.url().path {
            "/plaintext" | "/" => {
                builder = builder.header("content-type", "text/plain");
                "Hello, World!".to_string()
            }
            "/post" => {
                let body = req.body_mut();
                let mut buf = [0u8; 10];
                if let Ok(len) = body.read(&mut buf) {
                    println!("skip = {:?}", &buf[..len]);
                }
                let mut binary = BinaryMut::new();
                body.read_all(&mut binary).await.unwrap();
                println!("binary = {:?}", binary);
    
                builder = builder.header("content-type", "text/plain");
                format!("Hello, World! {:?}", TryInto::<String>::try_into(binary)).to_string()
            }
            _ => {
                builder = builder.status(404);
                String::new()
            }
        };
    
        let (sender, receiver) = channel(10);
        let recv = RecvStream::new(receiver, BinaryMut::from(body.into_bytes().to_vec()), false);
        let response = builder
            .body(recv)
            .map_err(|_err| io::Error::new(io::ErrorKind::Other, ""))?;
    
        tokio::spawn(async move {
            println!("send!!!!!");
            for i in 1..2 {
                sender
                    .send((false, Binary::from(format!("hello{} ", i).into_bytes())))
                    .await;
            }
            println!("send!!!!! end!!!!!!");
            // sender.send((true, Binary::from_static("world\r\n".as_bytes()))).await;
        });
        Ok(Some(response))
    }

    pub async fn process1<T>(self, inbound: T) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let build = Client::builder();

        // let create = ProtCreate::new(self.sock_map, Some(host_name));
        // let (stream_sender, stream_receiver) = channel::<ProtFrame>(10);
        // let _ = self.sender_work.send((create, stream_sender)).await;
        
        let (virtual_sender, virtual_receiver) = channel::<ProtFrame>(10);
        // let _ = self.sender_work.send((create, stream_sender)).await;
        let stream = VirtualStream::new(
            self.sock_map,
            self.sender.clone(),
            virtual_receiver,
        );
        let mut client = Client::new(build.value().ok().unwrap(), stream);
        let mut server = Server::new(inbound, (client, virtual_sender));

        let _ret = server.incoming(Self::operate).await;
        Ok(())
    }
}
