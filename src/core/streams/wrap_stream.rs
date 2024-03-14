use std::{fmt::Debug, net::SocketAddr, pin::Pin};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::INVALID_SOCKET_ADDR;

use super::{ClientAddrTrait, DescTrait, IoTrait, ListenAddrTrait};

#[derive(Debug)]
pub struct WrapStream<IO: AsyncRead + AsyncWrite + Unpin + Debug + Send + Sync + 'static> {
    io: IO,
    addr: SocketAddr,
    desc: &'static str,
    listen_addr: SocketAddr,
}

impl<IO: AsyncRead + AsyncWrite + Unpin + Debug + Send + Sync + 'static> WrapStream<IO> {
    pub fn new(io: IO) -> Self {
        Self {
            io,
            addr: INVALID_SOCKET_ADDR,
            desc: "",
            listen_addr: INVALID_SOCKET_ADDR,
        }
    }

    pub fn with_addr(io: IO, addr: SocketAddr) -> Self {
        Self {
            io,
            addr,
            desc: "",
            listen_addr: INVALID_SOCKET_ADDR,
        }
    }

    pub fn set_desc(&mut self, desc: &'static str) {
        self.desc = desc
    }
    
    pub fn set_listen_addr(&mut self, listen_addr: SocketAddr) {
        self.listen_addr = listen_addr
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin + Debug + Send + Sync + 'static> ListenAddrTrait
    for WrapStream<IO>
{
    fn listen_addr(&self) -> &SocketAddr {
        &self.listen_addr
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin + Debug + Send + Sync + 'static> ClientAddrTrait
    for WrapStream<IO>
{
    fn client_addr(&self) -> SocketAddr {
        self.addr
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin + Debug + Send + Sync + 'static> DescTrait
    for WrapStream<IO>
{
    fn desc(&self) -> &'static str {
        self.desc
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin + Debug + Send + Sync + 'static> IoTrait
    for WrapStream<IO>
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin + Debug + Send + Sync + 'static> AsyncRead
    for WrapStream<IO>
{
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let io = Pin::new(&mut Pin::get_mut(self).io);
        io.poll_read(cx, buf)
    }
}

impl<IO: AsyncRead + AsyncWrite + Unpin + Debug + Send + Sync + 'static> AsyncWrite
    for WrapStream<IO>
{
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let io = Pin::new(&mut Pin::get_mut(self).io);
        io.poll_write(cx, buf)
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let io = Pin::new(&mut Pin::get_mut(self).io);
        io.poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let io = Pin::new(&mut Pin::get_mut(self).io);
        io.poll_shutdown(cx)
    }
}
