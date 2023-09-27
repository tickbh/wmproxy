use std::collections::HashMap;
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt},
    sync::mpsc::{channel, Receiver},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::Sender,
};

use webparse::BinaryMut;
use webparse::Buf;

use crate::{
    prot::{ProtClose, ProtFrame},
    Helper, Proxy, ProxyOption, ProxyResult, VirtualStream,
};

pub struct CenterServer {
    option: ProxyOption,
    sender: Sender<ProtFrame>,
    receiver: Option<Receiver<ProtFrame>>,

    sender_work: Sender<(u32, Sender<ProtFrame>)>,
    receiver_work: Option<Receiver<(u32, Sender<ProtFrame>)>>,
    next_id: u32,
}

impl CenterServer {
    pub fn new(option: ProxyOption) -> Self {
        let (sender, receiver) = channel::<ProtFrame>(100);
        let (sender_work, mut receiver_work) = channel::<(u32, Sender<ProtFrame>)>(10);

        Self {
            option,
            sender,
            receiver: Some(receiver),
            sender_work,
            receiver_work: Some(receiver_work),
            next_id: 2,
        }
    }

    pub fn sender(&self) -> Sender<ProtFrame> {
        self.sender.clone()
    }
    
    pub fn sender_work(&self) -> Sender<(u32, Sender<ProtFrame>)> {
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
        mut receiver_work: Receiver<(u32, Sender<ProtFrame>)>,
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
                r = receiver_work.recv() => {
                    println!("center_client receiver = {:?}", r);
                    if let Some((sock, sender)) = r {
                        map.insert(sock, sender);
                        println!("write create socket");
                        let _ = ProtFrame::new_create(sock, None).encode(&mut write_buf);
                    }
                }
                r = receiver.recv() => {
                    println!("receiver = {:?}", r);
                    if let Some(p) = r {
                        let _ = p.encode(&mut write_buf);
                    }
                }
                r = reader.read(&mut vec) => {
                    println!("read = {:?}", r);
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
                match Helper::decode_frame(&mut read_buf)? {
                    Some(p) => {
                        println!("server receiver = {:?}", p);
                        if p.is_create() {
                            let (virtual_sender, virtual_receiver) = channel::<ProtFrame>(10);
                            map.insert(p.sock_map(), virtual_sender);
                            let stream =
                                VirtualStream::new(p.sock_map(), sender.clone(), virtual_receiver);

                            let (flag, username, password, udp_bind) = (
                                option.flag,
                                option.username.clone(),
                                option.password.clone(),
                                option.udp_bind.clone(),
                            );
                            tokio::spawn(async move {
                                let _ =
                                    Proxy::deal_proxy(stream, flag, username, password, udp_bind)
                                        .await;
                            });
                        } else if p.is_close() {
                            if let Some(sender) = map.get(&p.sock_map()) {
                                let _ = sender.try_send(p);
                            }
                        } else if p.is_data() {
                            if let Some(sender) = map.get(&p.sock_map()) {
                                let _ = sender.try_send(p);
                            }
                        }
                    }
                    None => {
                        break;
                    }
                }

                if !read_buf.has_remaining() {
                    read_buf.clear();
                }
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

        tokio::spawn(async move {
            let _ = Self::inner_serve(stream, option, sender, receiver, receiver_work).await;
        });
        Ok(())
    }
}
