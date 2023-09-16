use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs},
};

use crate::{ProxyError, ProxyResult};
use tokio::{
    io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt, ReadBuf},
    net::TcpStream,
};
use webparse::{BinaryMut, Buf, BufMut, HttpError, Method, WebError};

pub struct ProxySocks5 {
    username: Option<String>,
    password: Option<String>,
}

impl ProxySocks5 {
    pub const SOCK_CONNECT: u8 = 0x01u8;
    pub const SOCK_BIND: u8 = 0x02u8;
    pub const SOCK_UDP: u8 = 0x03u8;

    pub fn new(username: Option<String>, password: Option<String>) -> Self {
        Self { username, password }
    }
    pub async fn read_head_len(
        &self,
        stream: &mut TcpStream,
        buffer: &mut BinaryMut,
    ) -> ProxyResult<u8> {
        let _ = self.read_len(stream, buffer, 2).await;
        if buffer.get_u8() == 5 {
            return Err(ProxyError::SizeNotMatch);
        }
        let len = buffer.get_u8() as usize;
        let _ = self.read_len(stream, buffer, len).await;
        let mut verify = 0;
        let chunk = buffer.chunk();
        println!("len = {}, chunk = {:?}", len, buffer.chunk());
        if self.is_user_password() {
            if !chunk.contains(&2) {
                verify = 0xFF;
            } else {
                verify = 2u8;
            }
        }
        buffer.advance(len);
        return Ok(verify);
    }

    pub async fn read_verify(
        &self,
        stream: &mut TcpStream,
        buffer: &mut BinaryMut,
    ) -> ProxyResult<bool> {
        let _ = self.read_len(stream, buffer, 2).await?;
        if buffer.get_u8() == 1 {
            return Err(ProxyError::ProtocolErr);
        }
        let user_len = buffer.get_u8() as usize;
        let _ = self.read_len(stream, buffer, user_len).await?;
        if let Some(user) = &self.username {
            if user_len == 0 || user.as_bytes() != &buffer.chunk()[0..user_len] {
                return Ok(false);
            }
            buffer.advance(user_len);
        }
        let _ = self.read_len(stream, buffer, 1).await?;
        let pass_len = buffer.get_u8() as usize;
        let _ = self.read_len(stream, buffer, pass_len).await?;
        if let Some(user) = &self.username {
            if pass_len == 0 || user.as_bytes() != &buffer.chunk()[0..pass_len] {
                return Ok(false);
            }
            buffer.advance(pass_len);
        }
        Ok(true)
    }

    pub async fn read_len(
        &self,
        stream: &mut TcpStream,
        buffer: &mut BinaryMut,
        size: usize,
    ) -> ProxyResult<()> {
        buffer.reserve(size);
        loop {
            if buffer.remaining() >= size {
                return Ok(());
            }
            let n = {
                let mut buf = ReadBuf::uninit(buffer.chunk_mut());
                stream.read_buf(&mut buf).await?;
                buf.filled().len()
            };
            if n == 0 {
                return Err(ProxyError::IoError(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "",
                )));
            }
            unsafe {
                buffer.advance_mut(n);
            }
        }
    }

    pub async fn read_addr(
        &self,
        stream: &mut TcpStream,
        buffer: &mut BinaryMut,
    ) -> ProxyResult<(u8, SocketAddr)> {
        let _ = self.read_len(stream, buffer, 4).await?;
        if buffer.get_u8() == 5 {
            return Err(ProxyError::ProtocolErr);
        }
        let sock = buffer.get_u8();
        if buffer.get_u8() == 0 {
            return Err(ProxyError::ProtocolErr);
        }
        let atyp = buffer.get_u8();
        let addr = match atyp {
            0x01 => {
                self.read_len(stream, buffer, 6).await?;
                SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(
                        buffer.get_u8(),
                        buffer.get_u8(),
                        buffer.get_u8(),
                        buffer.get_u8(),
                    )),
                    buffer.get_u16(),
                )
            }
            0x03 => {
                self.read_len(stream, buffer, 1).await?;
                let len = buffer.get_u8() as usize;
                self.read_len(stream, buffer, len + 2).await?;
                let name = String::from_utf8_lossy(&buffer.chunk()[0..len]).to_string();
                buffer.advance(len);
                let port = buffer.get_u16();
                let domain = format!("{}:{}", name, port);
                let mut addrs = domain.to_socket_addrs()?;
                addrs.next().unwrap()
            }
            0x04 => {
                self.read_len(stream, buffer, 18).await?;
                SocketAddr::new(
                    IpAddr::V6(Ipv6Addr::new(
                        buffer.get_u16(),
                        buffer.get_u16(),
                        buffer.get_u16(),
                        buffer.get_u16(),
                        buffer.get_u16(),
                        buffer.get_u16(),
                        buffer.get_u16(),
                        buffer.get_u16(),
                    )),
                    buffer.get_u16(),
                )
            }
            _ => return Err(ProxyError::ProtocolErr),
        };
        return Ok((sock, addr));
    }

    pub async fn process(
        &mut self,
        stream: &mut TcpStream,
        buffer: Option<BinaryMut>,
    ) -> ProxyResult<()> {
        println!("socks5 process");
        let mut buffer = buffer.unwrap_or(BinaryMut::new());
        let verify = match self.read_head_len(stream, &mut buffer).await {
            Err(ProxyError::SizeNotMatch) => {
                return Err(ProxyError::Continue(Some(buffer)));
            }
            Err(err) => {
                return Err(err);
            }
            Ok(result) => result,
        };

        let is_verify = {
            stream.write_all(&[0x05_u8, verify]).await?;
            if verify == 0xFF {
                return Err(ProxyError::VerifyFail);
            }
            verify == 2
        };

        if is_verify {
            let succ = self.read_verify(stream, &mut buffer).await?;
            if !succ {
                stream.write_all(&[0x01_u8, 0x01]).await?;
                return Err(ProxyError::VerifyFail);
            } else {
                stream.write_all(&[0x01_u8, 0x00]).await?;
            }
        }

        let (_sock, addr) = self.read_addr(stream, &mut buffer).await?;
        println!("connecting {:?}", addr);
        let mut target = match TcpStream::connect(addr.clone()).await {
            Ok(tcp) => {
                stream.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
                tcp
            },
            Err(_err) => {
                stream.write_all(&[5, 1, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
                return Err(ProxyError::Extension("Can't connect tcp"));
            },
        };
        
        let _ = copy_bidirectional(stream, &mut target).await?;
        Ok(())
    }

    pub fn is_user_password(&self) -> bool {
        self.username.is_some() && self.password.is_some()
    }
}
