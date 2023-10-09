use std::{collections::HashMap, sync::Arc, net::SocketAddr};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{
        mpsc::{channel, Receiver},
        RwLock,
    },
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::Sender,
};

use tokio_rustls::TlsAcceptor;
use webparse::BinaryMut;
use webparse::Buf;

use crate::{
    prot::{ProtClose, ProtFrame},
    trans::{TransHttp, TransTcp},
    Helper, MappingConfig, ProtCreate, Proxy, ProxyOption, ProxyResult, VirtualStream,
};

/// 中心服务端
/// 接受中心客户端的连接，并且将信息处理或者转发
pub struct CenterServer {
    /// 代理的详情信息，如用户密码这类
    option: ProxyOption,

    /// 发送协议数据，接收到服务端的流数据，转发给相应的Stream
    sender: Sender<ProtFrame>,
    /// 接收协议数据，并转发到服务端。
    receiver: Option<Receiver<ProtFrame>>,

    /// 发送Create，并将绑定的Sender发到做绑定
    sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    /// 接收的Sender绑定，开始服务时这值move到工作协程中，所以不能二次调用服务
    receiver_work: Option<Receiver<(ProtCreate, Sender<ProtFrame>)>>,
    /// 绑定的下一个sock_map映射，为双数
    next_id: u32,

    /// 内网映射的相关消息, 需要读写分离需加锁
    mappings: Arc<RwLock<Vec<MappingConfig>>>,
}

impl CenterServer {
    pub fn new(option: ProxyOption) -> Self {
        let (sender, receiver) = channel::<ProtFrame>(100);
        let (sender_work, receiver_work) = channel::<(ProtCreate, Sender<ProtFrame>)>(10);

        Self {
            option,
            sender,
            receiver: Some(receiver),
            sender_work,
            receiver_work: Some(receiver_work),
            next_id: 2,
            mappings: Arc::new(RwLock::new(vec![])),
        }
    }

    pub fn sender(&self) -> Sender<ProtFrame> {
        self.sender.clone()
    }

    pub fn sender_work(&self) -> Sender<(ProtCreate, Sender<ProtFrame>)> {
        self.sender_work.clone()
    }

    pub fn is_close(&self) -> bool {
        self.sender.is_closed()
    }

    pub fn calc_next_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 2;
        id
    }

    pub async fn inner_serve<T>(
        stream: T,
        option: ProxyOption,
        sender: Sender<ProtFrame>,
        mut receiver: Receiver<ProtFrame>,
        mut receiver_work: Receiver<(ProtCreate, Sender<ProtFrame>)>,
        mappings: Arc<RwLock<Vec<MappingConfig>>>,
    ) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        println!("center_server {:?}", "aaaa");
        let mut map = HashMap::<u32, Sender<ProtFrame>>::new();
        let mut read_buf = BinaryMut::new();
        let mut write_buf = BinaryMut::new();

        let (mut reader, mut writer) = split(stream);
        let mut vec = vec![0u8; 4096];
        let is_closed;
        loop {
            let _ = tokio::select! {
                // 严格的顺序流
                biased;
                // 新的流建立，这里接收Create并进行绑定
                r = receiver_work.recv() => {
                    if let Some((create, sender)) = r {
                        map.insert(create.sock_map(), sender);
                        let _ = create.encode(&mut write_buf);
                    }
                }
                // 数据的接收，并将数据写入给远程端
                r = receiver.recv() => {
                    if let Some(p) = r {
                        let _ = p.encode(&mut write_buf);
                    }
                }
                // 数据的等待读取，一旦流可读则触发，读到0则关闭主动关闭所有连接
                r = reader.read(&mut vec) => {
                    match r {
                        Ok(0)=>{
                            is_closed=true;
                            break;
                        }
                        Ok(n) => {
                            read_buf.put_slice(&vec[..n]);
                        }
                        Err(_) => {
                            is_closed = true;
                            break;
                        },
                    }
                }
                // 一旦有写数据，则尝试写入数据，写入成功后扣除相应的数据
                r = writer.write(write_buf.chunk()), if write_buf.has_remaining() => {
                    println!("write = {:?}", r);
                    match r {
                        Ok(n) => {
                            write_buf.advance(n);
                            if !write_buf.has_remaining() {
                                write_buf.clear();
                            }
                        }
                        Err(_) => todo!(),
                    }
                }
            };

            loop {
                // 将读出来的数据全部解析成ProtFrame并进行相应的处理，如果是0则是自身消息，其它进行转发
                match Helper::decode_frame(&mut read_buf)? {
                    Some(p) => {
                        println!("server decode receiver = {:?}", p);
                        match p {
                            ProtFrame::Create(p) => {
                                let (virtual_sender, virtual_receiver) = channel::<ProtFrame>(10);
                                map.insert(p.sock_map(), virtual_sender);
                                let stream = VirtualStream::new(
                                    p.sock_map(),
                                    sender.clone(),
                                    virtual_receiver,
                                );

                                let (flag, username, password, udp_bind) = (
                                    option.flag,
                                    option.username.clone(),
                                    option.password.clone(),
                                    option.udp_bind.clone(),
                                );
                                tokio::spawn(async move {
                                    // 处理代理的能力
                                    let _ = Proxy::deal_proxy(
                                        stream, flag, username, password, udp_bind,
                                    )
                                    .await;
                                });
                            }
                            ProtFrame::Close(_) | ProtFrame::Data(_) => {
                                if let Some(sender) = map.get(&p.sock_map()) {
                                    let _ = sender.send(p).await;
                                }
                            }
                            ProtFrame::Mapping(p) => {
                                let mut guard = mappings.write().await;
                                *guard = p.into_mappings();
                                println!("new mapping is =  {:?}", *guard);
                            }
                        }
                    }
                    None => {
                        break;
                    }
                }
            }
            if !read_buf.has_remaining() {
                read_buf.clear();
            }
        }
        if is_closed {
            for v in map {
                let _ = v.1.try_send(ProtFrame::Close(ProtClose::new(v.0)));
            }
        }
        Ok(())
    }

    pub async fn serve<T>(&mut self, stream: T) -> ProxyResult<()>
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        if self.receiver.is_none() || self.receiver_work.is_none() {
            println!("receiver is none");
            return Ok(());
        }
        let option = self.option.clone();
        let sender = self.sender.clone();
        let receiver = self.receiver.take().unwrap();
        let receiver_work = self.receiver_work.take().unwrap();
        let mapping = self.mappings.clone();
        tokio::spawn(async move {
            let _ =
                Self::inner_serve(stream, option, sender, receiver, receiver_work, mapping).await;
        });
        Ok(())
    }

    pub async fn server_new_http(&mut self, stream: TcpStream, addr: SocketAddr) -> ProxyResult<()> {
        println!("server_new_http!!!!!!!!!!!! =====");
        let trans = TransHttp::new(self.sender(), self.sender_work(), self.calc_next_id(), self.mappings.clone());
        tokio::spawn(async move {
            println!("tokio::spawn start!");
            let e = trans.process(stream, addr).await;
            println!("tokio::spawn end! = {:?}", e);
        });
        return Ok(());
    }

    pub async fn server_new_https(&mut self, stream: TcpStream, addr: SocketAddr, accept: TlsAcceptor) -> ProxyResult<()> {
        let trans = TransHttp::new(self.sender(), self.sender_work(), self.calc_next_id(), self.mappings.clone());
        tokio::spawn(async move {
            println!("tokio::spawn start!");
            if let Ok(tls_stream) = accept.accept(stream).await {
                let e = trans.process(tls_stream, addr).await;
                println!("tokio::spawn end! = {:?}", e);
            }
        });
        return Ok(());
    }

    pub async fn server_new_tcp(&mut self, stream: TcpStream) -> ProxyResult<()> {
        let trans = TransTcp::new(self.sender(), self.sender_work(), self.calc_next_id(), self.mappings.clone());
        tokio::spawn(async move {
            println!("tokio::spawn start!");
            let e = trans.process(stream).await;
            println!("tokio::spawn end! = {:?}", e);
        });
        return Ok(());
    }
}
