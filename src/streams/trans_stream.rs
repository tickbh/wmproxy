use std::{
    pin::Pin,
    task::{ready, Context, Poll}, io, collections::LinkedList,
};


use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf, split, AsyncWriteExt},
    sync::mpsc::{Receiver, Sender},
};
use webparse::{BinaryMut, Buf, BufMut};

use crate::{ProtFrame};

/// 转发流量端
/// 提供与中心端绑定的读出写入功能
pub struct TransStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    // 流有相应的AsyncRead + AsyncWrite + Unpin均可
    stream: T,
    // sock绑定的句柄
    id: u32,
    // 读取的数据缓存，将转发成ProtFrame
    read: BinaryMut,
    // 写的数据缓存，直接写入到stream下，从ProtFrame转化而来
    write: BinaryMut,
    // 收到数据通过sender发送给中心端
    in_sender: Sender<ProtFrame>,
    // 收到中心端的写入请求，转成write
    out_receiver: Receiver<ProtFrame>,
}

impl<T> TransStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(
        stream: T,
        id: u32,
        in_sender: Sender<ProtFrame>,
        out_receiver: Receiver<ProtFrame>,
    ) -> Self {
        Self {
            stream,
            id,
            read: BinaryMut::new(),
            write: BinaryMut::new(),
            in_sender,
            out_receiver,
        }
    }

    pub fn reader_mut(&mut self) -> &mut BinaryMut {
        &mut self.read
    }

    
    pub fn write_mut(&mut self) -> &mut BinaryMut {
        &mut self.read
    }

    async fn inner_copy_wait(mut self) -> Result<(), std::io::Error> {
        let mut buf = Vec::with_capacity(20480);
        buf.resize(20480, 0);
        let mut link = LinkedList::<ProtFrame>::new();
        let (mut reader, mut writer) = split(self.stream);
        loop {
            // 有剩余数据，优先转化成Prot，因为数据可能从外部直接带入
            if self.read.has_remaining() {
                link.push_back(ProtFrame::new_data(self.id, self.read.copy_to_binary()));
                self.read.clear();
            }

            tokio::select! {
                n = reader.read(&mut buf) => {
                    let n = n?;
                    if n == 0 {
                        return Ok(())
                    } else {
                        self.read.put_slice(&buf[..n]);
                    }
                },
                r = writer.write(self.write.chunk()), if self.write.has_remaining() => {
                    match r {
                        Ok(n) => {
                            self.write.advance(n);
                            if !self.write.has_remaining() {
                                self.write.clear();
                            }
                        }
                        Err(_) => todo!(),
                    }
                }
                r = self.out_receiver.recv() => {
                    if let Some(v) = r {
                        if v.is_close() || v.is_create() {
                            return Ok(())
                        } else if v.is_data() {
                            match v {
                                ProtFrame::Data(d) => {
                                    self.write.put_slice(&d.data().chunk());
                                }
                                _ => unreachable!(),
                            }
                        }
                    } else {
                        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid frame"))
                    }
                }
                p = self.in_sender.reserve(), if link.len() > 0 => {
                    match p {
                        Err(_)=>{
                            return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid frame"))
                        }
                        Ok(p) => {
                            p.send(link.pop_front().unwrap())
                        }, 
                    }
                }
            }
        }
    }

    pub async fn copy_wait(self) -> Result<(), std::io::Error> {
        let sender = self.in_sender.clone();
        let id = self.id;
        let ret = self.inner_copy_wait().await;
        let _ = sender.send(ProtFrame::new_close(id)).await;
        ret
    }

    pub fn stream_read(&mut self, cx: &mut Context<'_>) -> Poll<std::io::Result<usize>> {
        self.read.reserve(1);
        let n = {
            let mut buf = ReadBuf::uninit(self.read.chunk_mut());
            let ptr = buf.filled().as_ptr();
            ready!(Pin::new(&mut self.stream).poll_read(cx, &mut buf)?);
            assert_eq!(ptr, buf.filled().as_ptr());
            buf.filled().len()
        };

        unsafe {
            self.read.advance_mut(n);
        }
        Poll::Ready(Ok(n))
    }


}

impl<T> AsyncRead for TransStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if !self.read.has_remaining() {
            ready!(self.stream_read(cx))?;
        }
        if self.read.has_remaining() {
            let copy = std::cmp::min(self.read.remaining(), buf.remaining());
            buf.put_slice(&self.read.chunk()[..copy]);
            self.read.advance(copy);
            return Poll::Ready(Ok(()));
        }
        return Poll::Ready(Ok(()));
    }
}

impl<T> AsyncWrite for TransStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

