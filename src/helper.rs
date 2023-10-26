use std::{io, net::ToSocketAddrs};

use socket2::{Socket, Domain, Type};
use tokio::net::{TcpListener};
use webparse::{BinaryMut, Buf, http2::frame::read_u24};

use crate::{ProxyResult, prot::{ProtFrame, ProtFrameHeader}};

pub struct Helper;

impl Helper {
    pub fn decode_frame(read: &mut BinaryMut) -> ProxyResult<Option<ProtFrame>> {
        let data_len = read.remaining();
        if data_len < 8 {
            return Ok(None);
        }
        let mut copy = read.clone();
        let length = read_u24(&mut copy);
        let all_len = length as usize + ProtFrameHeader::FRAME_HEADER_BYTES;
        if all_len > data_len {
            return Ok(None);
        }
        read.advance(all_len);
        copy.mark_len(all_len - 3);
        let header = match ProtFrameHeader::parse_by_len(&mut copy, length) {
            Ok(v) => v,
            Err(err) => return Err(err),
        };

        match ProtFrame::parse(header, copy) {
            Ok(v) => return Ok(Some(v)),
            Err(err) => return Err(err),
        };
    }

    #[cfg(not(target_os = "windows"))]
    fn set_reuse_port(socket: &Socket, reuse: bool) -> io::Result<()> {
        socket.set_reuse_port(true)?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn set_reuse_port(_socket: &Socket, _sreuse: bool) -> io::Result<()> {
        Ok(())
    }
    
    pub async fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;
        for addr in addrs {
            let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
            socket.set_nonblocking(true)?;
            let _ = socket.set_only_v6(false);
            socket.set_reuse_address(true)?;
            Self::set_reuse_port(&socket, true)?;
            socket.bind(&addr.into())?;
            match socket.listen(128) {
                Ok(_) => {
                    let listener: std::net::TcpListener = socket.into();
                    return TcpListener::from_std(listener);
                }
                Err(e) => {
                    log::info!("绑定端口地址失败，原因： {:?}", addr);
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "could not resolve to any address",
            )
        }))
    }
}