use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use tokio::io::{AsyncRead, AsyncWrite};

mod wrap_stream;
pub use wrap_stream::WrapStream;

#[async_trait]
pub trait Shutdown {
    async fn shutdown(&mut self) -> ();
}

pub trait UniqueID {
    fn id(&self) -> i32;
}

pub trait IoTrait: AsyncRead + AsyncWrite + Unpin + Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

pub type Stream = Box<dyn IoTrait>;
