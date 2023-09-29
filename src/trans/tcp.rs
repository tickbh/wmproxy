use std::sync::Arc;

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    sync::{mpsc::{Sender, channel}, RwLock},
};
use webparse::{BinaryMut, Buf, BufMut, HttpError, WebError, http::{response, StatusCode}, Response};

use crate::{ProtFrame, TransStream, ProxyError, ProtCreate, MappingConfig};

pub struct TransTcp {
    sender: Sender<ProtFrame>,
    sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    sock_map: u32,
    mappings: Arc<RwLock<Vec<MappingConfig>>>,
}

impl TransTcp {
    pub fn new(
        sender: Sender<ProtFrame>,
        sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
        sock_map: u32,
        mappings: Arc<RwLock<Vec<MappingConfig>>>,
    ) -> Self {
        Self {
            sender,
            sender_work,
            sock_map,
            mappings,
        }
    }

    async fn err_server_status<T>(mut inbound: T, status: u16) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut res = webparse::Response::builder().status(status).body(())?;
        inbound.write_all(&res.httpdata()?).await?;
        Ok(())
    }

    pub async fn process<T>(self, inbound: T) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        {
            let mut is_find = false;
            let read = self.mappings.read().await;
            for v in &*read {
                if v.mode == "tcp" {
                    is_find = true;
                }
            }
            if !is_find {
                log::warn!("not found tcp client trans");
                return Ok(());
            }
        }

        let create = ProtCreate::new(self.sock_map, None);
        let (stream_sender, stream_receiver) = channel::<ProtFrame>(10);
        let _ = self.sender_work.send((create, stream_sender)).await;
        
        println!("ending!!!!!! create");
        let trans = TransStream::new(inbound, self.sock_map, self.sender, stream_receiver);
        trans.copy_wait().await?;
        println!("ending!!!!!! copy");
        // let _ = copy_bidirectional(&mut inbound, &mut outbound).await?;
        Ok(())
    }
}
