use std::{collections::HashMap};
use tokio::{
    io::{split, AsyncReadExt, AsyncWriteExt},
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::mpsc::Receiver,
    sync::mpsc::Sender,
};

use webparse::BinaryMut;
use webparse::Buf;

use crate::{
    prot::{ProtClose, ProtFrame},
    ProxyResult,
};

pub struct CenterServer<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    stream: T,
    sender: Option<Sender<ProtFrame>>,
    receiver: Option<Receiver<ProtFrame>>,
}

impl<T> CenterServer<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: T) -> Self {
        Self {
            stream,
            sender: None,
            receiver: None,
        }
    }

    pub async fn inner_serve(
        stream: T,
        receiver_work: &mut Receiver<(u32, Sender<ProtFrame>)>,
        receiver: &mut Receiver<ProtFrame>,
    ) -> ProxyResult<()> {
        let mut map = HashMap::<u32, Sender<ProtFrame>>::new();
        let mut read_buf = BinaryMut::new();
        let mut write_buf = BinaryMut::new();
        let (mut reader, mut writer) = split(stream);
        let mut vec = vec![0u8; 4096];
        let is_closed;
        loop {
            let _ = tokio::select! {
                r = receiver_work.recv() => {
                    if let Some((sock, sender)) = r {
                        map.insert(sock, sender);
                    }
                }
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
        }
        if is_closed {
            for v in map {
                let _ = v.1.try_send(ProtFrame::Close(ProtClose::new(v.0)));
            }
        }
        Ok(())
    }

    pub async fn serve(&mut self) {
        // let stream = self.stream;

        tokio::spawn(async move {
            // Self::inner_serve(stream, receiver_work, receiver);
        });
    }
}
