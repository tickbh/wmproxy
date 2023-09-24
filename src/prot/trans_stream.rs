use std::{
    pin::Pin,
    task::{ready, Context, Poll},
};

use futures_core::Stream;
use tokio::{io::{AsyncRead, AsyncWrite, ReadBuf}, sync::mpsc::{Sender, Receiver}};
use webparse::{http2::frame::read_u24, BinaryMut, Buf, BufMut};

use crate::ProxyResult;

use super::{ProtFrame, ProtFrameHeader};

pub struct TransStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    stream: T,
    read: BinaryMut,
    write: BinaryMut,
    in_sender: Option<Sender<ProtFrame>>,
    out_receiver: Option<Receiver<ProtFrame>>,
}

impl<T> TransStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: T) -> Self {
        Self {
            stream,
            read: BinaryMut::new(),
            write: BinaryMut::new(),
            in_sender: None,
            out_receiver: None,
        }
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

    pub fn poll_read_all(&mut self, cx: &mut Context<'_>) -> Poll<std::io::Result<usize>> {
        let mut size = 0;
        loop {
            match self.stream_read(cx)? {
                Poll::Ready(0) => return Poll::Ready(Ok(0)),
                Poll::Ready(n) => size += n,
                Poll::Pending => {
                    if size == 0 {
                        return Poll::Pending;
                    } else {
                        break;
                    }
                }
            }
        }
        Poll::Ready(Ok(size))
    }

    pub fn decode_frame(&mut self) -> ProxyResult<Option<ProtFrame>> {
        let data_len = self.read.remaining();
        if data_len < 8 {
            return Ok(None);
        }
        let mut copy = self.read.clone();
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

impl<T> Stream for TransStream<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
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
