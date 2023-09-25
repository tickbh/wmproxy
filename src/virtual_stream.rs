use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_core::Stream;
use tokio::{io::{AsyncRead, AsyncWrite, ReadBuf}, sync::mpsc::{Sender, Receiver}};
use webparse::{http2::frame::read_u24, BinaryMut, Buf, BufMut};

use crate::{ProxyResult, prot::ProtFrame};

pub struct VirtualStream
{
    sender: Sender<ProtFrame>,
    receiver: Receiver<ProtFrame>,
    read: BinaryMut,
    write: BinaryMut,
}

impl VirtualStream
{
    pub fn new(sender: Sender<ProtFrame>, receiver: Receiver<ProtFrame>) -> Self {
        Self {
            sender,
            receiver,
            read: BinaryMut::new(),
            write: BinaryMut::new(),
        }
    }
}

impl AsyncRead for VirtualStream
{
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        loop {
            match self.receiver.poll_recv(cx) {
                Poll::Ready(value) => {
                    if let Some(v) = value {
                        if v.is_close() || v.is_create() {
                            return Poll::Ready(Ok(()))
                        } else if v.is_data() {
                            match v {
                                ProtFrame::Data(d) => {
                                    self.read.put_slice(&d.data().chunk());
                                }
                                _ => unreachable!(),
                            }
                            // self.read.put_slice()
                        }
                    } else {
                        return Poll::Ready(Ok(()))
                    }
                },
                Poll::Pending => {
                    if self.read.has_remaining() {
                        return Poll::Pending;
                    }
                },
            }


            if self.read.has_remaining() {
                let copy = std::cmp::min(self.read.remaining(), buf.remaining());
                buf.put_slice(&self.read.chunk()[..copy]);
                self.read.advance(copy);
                return Poll::Ready(Ok(()));
            }
        }
        
    }
}

impl AsyncWrite for VirtualStream
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        self.write.put_slice(buf);
        // self.sender.try_reserve()
        // self.sender.poll_

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

impl Stream for VirtualStream
{
    type Item = ProxyResult<ProtFrame>;
    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if let Some(v) = self.decode_frame()? {
            return Poll::Ready(Some(Ok(v)));
        }
        match ready!(self.poll_read_all(cx)?) {
            0 => {
                println!("test:::: recv client end!!!");
                return Poll::Ready(None);
            }
            _ => {
                if let Some(v) = self.decode_frame()? {
                    return Poll::Ready(Some(Ok(v)));
                } else {
                    return Poll::Pending;
                }
            }
        }
    }
}
