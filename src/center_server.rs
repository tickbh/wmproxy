use std::collections::HashMap;
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt},
    sync::mpsc::channel,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::Receiver,
    sync::mpsc::Sender,
};

use webparse::Buf;
use webparse::{http2::frame::read_u24, BinaryMut};

use crate::{
    prot::{ProtClose, ProtFrame, ProtFrameHeader},
    ProxyOption, ProxyResult, VirtualStream, Proxy,
};

pub struct CenterServer<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    stream: T,
    option: ProxyOption,
}

impl<T> CenterServer<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static
{
    pub fn new(stream: T, option: ProxyOption) -> Self {
        Self { stream, option }
    }

    pub async fn inner_serve(stream: T, option: ProxyOption) -> ProxyResult<()> {
        let mut map = HashMap::<u32, Sender<ProtFrame>>::new();
        let mut read_buf = BinaryMut::new();
        let mut write_buf = BinaryMut::new();
        let (sender, mut receiver) = channel::<ProtFrame>(10);

        let (mut reader, mut writer) = split(stream);
        let mut vec = vec![0u8; 4096];
        let is_closed;
        loop {
            let _ = tokio::select! {
                r = receiver.recv() => {
                    if let Some(p) = r {
                        let _ = p.encode(&mut write_buf);
                    }
                }
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
                r = writer.write(write_buf.chunk()), if write_buf.has_remaining() => {
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
                match Self::decode_frame(&mut read_buf)? {
                    Some(p) => {
                        if p.is_create() {
                            let (virtual_sender, virtual_receiver) = channel::<ProtFrame>(10);
                            map.insert(p.sock_map(), virtual_sender);
                            let stream =
                                VirtualStream::new(p.sock_map(), sender.clone(), virtual_receiver);

                            let (flag, username, password, udp_bind) = (option.flag, option.username.clone(), option.password.clone(), option.udp_bind.clone());
                            tokio::spawn(async move {
                                let _ = Proxy::deal_proxy(stream, flag, username, password, udp_bind).await;
                            });
                        } else if p.is_close() {
                            if let Some(sender) = map.get(&p.sock_map()) {
                                sender.try_send(p);
                            }
                        } else if p.is_data() {
                            if let Some(sender) = map.get(&p.sock_map()) {
                                sender.try_send(p);
                            }
                        }
                    }
                    None => {
                        break;
                    }
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

    pub fn decode_frame(read: &mut BinaryMut) -> ProxyResult<Option<ProtFrame>> {
        let data_len = read.remaining();
        if data_len < 8 {
            return Ok(None);
        }
        let mut copy = read.clone();
        let length = read_u24(&mut copy);
        if length as usize > data_len {
            return Ok(None);
        }
        copy.mark_len(length as usize - 3);
        let header = match ProtFrameHeader::parse_by_len(&mut copy, length) {
            Ok(v) => v,
            Err(err) => return Err(err),
        };

        match ProtFrame::parse(header, copy) {
            Ok(v) => return Ok(Some(v)),
            Err(err) => return Err(err),
        };
    }

    pub async fn serve(self) {
        let stream = self.stream;
        let option = self.option;
        tokio::spawn(async move {
            let _ = Self::inner_serve(stream, option).await;
        });
    }
}
