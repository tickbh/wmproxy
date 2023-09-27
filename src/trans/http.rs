use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::TcpStream,
    sync::mpsc::{Sender, channel},
};
use webparse::{BinaryMut, Buf, BufMut, HttpError, Method, WebError};

use crate::{prot::{ProtFrame, TransStream}, ProxyError};

pub struct TransHttp {
    sender: Sender<ProtFrame>,
    sender_work: Sender<(ProtFrame, Sender<ProtFrame>)>,
    sock_map: u32,
}

impl TransHttp {
    pub fn new(
        sender: Sender<ProtFrame>,
        sender_work: Sender<(ProtFrame, Sender<ProtFrame>)>,
        sock_map: u32,
    ) -> Self {
        Self {
            sender,
            sender_work,
            sock_map,
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

    pub async fn process<T>(mut self, mut inbound: T) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut request;
        let mut host_name;
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
                Ok(s) => match request.get_connect_url() {
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
                    return Err(ProxyError::Continue((Some(buffer), inbound)));
                }
            }
        }

        let create = ProtFrame::new_create(self.sock_map, Some(host_name));
        let (stream_sender, stream_receiver) = channel::<ProtFrame>(10);
        let _ = self.sender_work.send((create, stream_sender)).await;
        
        let mut trans = TransStream::new(inbound, self.sock_map, Some(self.sender), Some(stream_receiver));
        trans.reader_mut().put_slice(buffer.chunk());
        trans.copy_wait().await?;
        // let _ = copy_bidirectional(&mut inbound, &mut outbound).await?;
        Ok(())
    }
}
