use async_trait::async_trait;
use std::{any::Any, net::SocketAddr};
use std::fmt::Debug;
use tokio::io::{AsyncRead, AsyncWrite};

mod wrap_stream;
pub use wrap_stream::WrapStream;

pub trait ClientAddrTrait {
    fn client_addr(&self) -> &Option<SocketAddr>;
}

pub trait IoTrait: AsyncRead + AsyncWrite + Unpin + ClientAddrTrait + Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

pub type Stream = Box<dyn IoTrait>;
