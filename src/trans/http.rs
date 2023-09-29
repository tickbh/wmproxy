use std::sync::Arc;

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    sync::{mpsc::{Sender, channel}, RwLock},
};
use webparse::{BinaryMut, Buf, BufMut, HttpError, WebError, http::{response, StatusCode}, Response};

use crate::{ProtFrame, TransStream, ProxyError, ProtCreate, MappingConfig};

pub struct TransHttp {
    sender: Sender<ProtFrame>,
    sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    sock_map: u32,
    mappings: Arc<RwLock<Vec<MappingConfig>>>,
}

impl TransHttp {
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

    pub async fn process<T>(self, mut inbound: T) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut request;
        let host_name;
        let mut buffer = BinaryMut::new();
        let mut cost_size = 0;
        loop {
            let size = {
                let mut buf = ReadBuf::uninit(buffer.chunk_mut());
                inbound.read_buf(&mut buf).await?;
                buf.filled().len()
            };

            if size == 0 {
                return Err(ProxyError::Extension("empty"));
            }
            unsafe {
                buffer.advance_mut(size);
            }
            request = webparse::Request::new();
            // 通过该方法解析标头是否合法, 若是partial(部分)则继续读数据
            // 若解析失败, 则表示非http协议能处理, 则抛出错误
            // 此处clone为浅拷贝，不确定是否一定能解析成功，不能影响偏移
            match request.parse_buffer(&mut buffer.clone()) {
                Ok(s) => match request.get_host() {
                    Some(host) => {
                        host_name = host;
                        cost_size = s;
                        break;
                    }
                    None => {
                        if !request.is_partial() {
                            Self::err_server_status(inbound, 503).await?;
                            return Err(ProxyError::UnknownHost);
                        }
                    }
                },
                Err(WebError::Http(HttpError::Partial)) => {
                    continue;
                }
                Err(_) => {
                    println!("buffer === {:?}", String::from_utf8_lossy(buffer.chunk()));
                    return Err(ProxyError::Continue((Some(buffer), inbound)));
                }
            }
        }

        {
            let mut is_find = false;
            let read = self.mappings.read().await;
            for v in &*read {
                if v.domain == host_name {
                    is_find = true;
                }
            }
            if !is_find {
                let mut res = Response::builder().status(404).body("no found ")?;
                let data = res.httpdata()?;
                inbound.write_all(&data).await?;
                return Ok(());
            }
        }

        let create = ProtCreate::new(self.sock_map, Some(host_name));
        let (stream_sender, stream_receiver) = channel::<ProtFrame>(10);
        let _ = self.sender_work.send((create, stream_sender)).await;
        
        println!("ending!!!!!! create");
        let mut trans = TransStream::new(inbound, self.sock_map, self.sender, stream_receiver);
        trans.reader_mut().put_slice(buffer.chunk());
        trans.copy_wait().await?;
        println!("ending!!!!!! copy");
        // let _ = copy_bidirectional(&mut inbound, &mut outbound).await?;
        Ok(())
    }
}
