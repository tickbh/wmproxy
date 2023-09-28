use std::{
    pin::Pin,
    task::{ready, Poll},
};
use tokio_util::sync::PollSender;

use tokio::{io::{AsyncRead, AsyncWrite}, sync::mpsc::{Sender, Receiver}};
use webparse::{Binary, BinaryMut, Buf, BufMut};

use crate::prot::ProtData;
use crate::{prot::ProtFrame};

pub struct VirtualStream
{
    id: u32,
    sender: PollSender<ProtFrame>,
    receiver: Receiver<ProtFrame>,
    read: BinaryMut,
    write: BinaryMut,
}

impl VirtualStream
{
    pub fn new(id: u32, sender: Sender<ProtFrame>, receiver: Receiver<ProtFrame>) -> Self {
        Self {
            id,
            sender: PollSender::new(sender),
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
                        }
                    } else {
                        return Poll::Ready(Ok(()))
                    }
                },
                Poll::Pending => {
                    if !self.read.has_remaining() {
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
        if let Err(_) = ready!(self.sender.poll_reserve(cx)) {
            return Poll::Pending;
        }
        let binary = Binary::from(self.write.chunk().to_vec());
        let id = self.id;
        if let Ok(_) = self.sender.send_item(ProtFrame::Data(ProtData::new(id, binary))) {
            self.write.clear();
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        if self.write.has_remaining() {
            if let Err(_) = ready!(self.sender.poll_reserve(cx)) {
                return Poll::Pending;
            }
            let binary = Binary::from(self.write.chunk().to_vec());
            let id = self.id;
            if let Ok(_) = self.sender.send_item(ProtFrame::Data(ProtData::new(id, binary))) {
                self.write.clear();
            }
        }
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        Poll::Ready(Ok(()))
    }
}
