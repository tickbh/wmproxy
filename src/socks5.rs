use crate::{ProxyError, ProxyResult};
use tokio::{
    io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt, ReadBuf},
    net::TcpStream,
};
use webparse::{BinaryMut, Buf, BufMut, Method, WebError, HttpError};

pub struct ProxySocks5 {
    username: Option<String>,
    password: Option<String>,
}

impl ProxySocks5 {
    pub async fn read_head_len(stream: &mut TcpStream, buffer: &mut BinaryMut) -> ProxyResult<()> {
        let mut size = 257;
        buffer.reserve(size);
        loop {
            if buffer.remaining() > size {
                return Err(ProxyError::SizeNotMatch);
            }
            if buffer.remaining() == size {
                return Ok(());
            }
            let n = {
                let mut buf = ReadBuf::uninit(buffer.chunk_mut());
                stream.read_buf(&mut buf).await?;
                buf.filled().len()
            };
            unsafe {
                buffer.advance_mut(n);
            }
            if buffer.len() > 2 {
                if buffer.chunk()[0] != 0x05 {
                    return Err(ProxyError::SizeNotMatch)
                }
                size = buffer.chunk()[1] as usize;
            }
        }
    }

    pub async fn read_len(stream: &mut TcpStream, buffer: &mut BinaryMut, size: usize) -> ProxyResult<()> {
        buffer.reserve(size);
        loop {
            if buffer.remaining() > size {
                return Err(ProxyError::SizeNotMatch);
            }
            if buffer.remaining() == size {
                return Ok(());
            }
            let n = {
                let mut buf = ReadBuf::uninit(buffer.chunk_mut());
                stream.read_buf(&mut buf).await?;
                buf.filled().len()
            };
            unsafe {
                buffer.advance_mut(n);
            }
        }
    }

    pub async fn process(stream: &mut TcpStream, buffer: Option<BinaryMut>) -> ProxyResult<()> {
        let mut buffer = buffer.unwrap_or(BinaryMut::new());
        match Self::read_head_len(stream, &mut buffer).await {
            Err(ProxyError::SizeNotMatch) => {
                return Err(ProxyError::Continue(Some(buffer)));
            }
            Err(err) => {
                return Err(err);
            }
            Ok(_) => (),
        }

        stream.write_all(&[0x05_u8, 0x00_u8]).await?;


        // let mut outbound;
        // let mut request;
        // let mut buffer = BinaryMut::new();
        // loop {
        //     let size = {
        //         let mut buf = ReadBuf::uninit(buffer.chunk_mut());
        //         stream.read_buf(&mut buf).await?;
        //         buf.filled().len()
        //     };

        //     if size == 0 {
        //         return Err(ProxyError::Extension("empty"));
        //     }
        //     unsafe {
        //         buffer.advance_mut(size);
        //     }
        //     request = webparse::Request::new();
        //     match request.parse_buffer(&mut buffer.clone()) {
        //         Ok(_) => {
        //             match request.get_connect_url() {
        //                 Some(host) => {
        //                     outbound = TcpStream::connect(host).await?;
        //                     break;
        //                 }
        //                 None => {
        //                     if !request.is_partial() {
        //                         return Err(ProxyError::UnknowHost);
        //                     }
        //                 }
        //             }
        //         },
        //         Err(WebError::Http(HttpError::Partial)) => {
        //             continue;
        //         },
        //         Err(_) => {
        //             return Err(ProxyError::Continue(buffer));
        //         }
        //     }
        // }

        // match request.method() {
        //     &Method::Connect => {
        //         println!("connect = {:?}", String::from_utf8_lossy(buffer.chunk()));
        //         stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
        //     }
        //     _ => {
        //         outbound.write_all(buffer.chunk()).await?;
        //     }
        // }
        // let _ = copy_bidirectional(stream, &mut outbound).await?;
        Ok(())
    }
}
