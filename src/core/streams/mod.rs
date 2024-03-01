use async_trait::async_trait;
use std::any::Any;
use std::fmt::Debug;
use tokio::io::{AsyncRead, AsyncWrite};

#[async_trait]
pub trait Shutdown {
    async fn shutdown(&mut self) -> ();
}

pub trait UniqueID {
    fn id(&self) -> i32;
}

pub trait IO: AsyncRead + AsyncWrite + Shutdown + UniqueID + Unpin + Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

pub type Stream = Box<dyn IO>;
