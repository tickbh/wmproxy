use crate::{ProxyError, ProxyResult};
use tokio::{
    io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt, ReadBuf},
    net::TcpStream,
};
use webparse::{BinaryMut, Buf, BufMut, Method, WebError, HttpError};

pub struct ProxyHttp {}

impl ProxyHttp {
    pub async fn process(inbound: &mut TcpStream) -> ProxyResult<bool> {
        let mut outbound;
        let mut request;
        let mut buffer = BinaryMut::new();
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
            match request.parse_buffer(&mut buffer.clone()) {
                Ok(_) => {
                    match request.get_connect_url() {
                        Some(host) => {
                            outbound = TcpStream::connect(host).await?;
                            break;
                        }
                        None => {
                            if !request.is_partial() {
                                return Err(ProxyError::UnknowHost);
                            }
                        }
                    }
                },
                Err(WebError::Http(HttpError::Partial)) => {
                    continue;
                },
                Err(_) => {
                    return Err(ProxyError::Continue(buffer));
                }
            }
        }

        match request.method() {
            &Method::Connect => {
                println!("connect = {:?}", String::from_utf8_lossy(buffer.chunk()));
                inbound.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
            }
            _ => {
                outbound.write_all(buffer.chunk()).await?;
            }
        }
        let _ = copy_bidirectional(inbound, &mut outbound).await?;
        Ok(true)
    }
}
