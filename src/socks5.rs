use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs},
};

use crate::{error::ProxyTypeResult, ProxyError, ProxyResult};
use tokio::{
    io::{copy_bidirectional, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf},
    net::{TcpStream, UdpSocket},
    sync::broadcast::{channel, Receiver, Sender},
    try_join,
};
use webparse::{BinaryMut, Buf, BufMut};

pub struct ProxySocks5 {
    username: Option<String>,
    password: Option<String>,
    bind_ip: Option<IpAddr>,
}

pub const SOCK_CONNECT: u8 = 0x01u8;
pub const SOCK_BIND: u8 = 0x02u8;
pub const SOCK_UDP: u8 = 0x03u8;

pub const SOCKS5_VERSION: u8 = 0x05;
pub const SOCKS5_ADDR_TYPE_IPV4: u8 = 0x01;
pub const SOCKS5_ADDR_TYPE_DOMAIN: u8 = 0x03;
pub const SOCKS5_ADDR_TYPE_IPV6: u8 = 0x04;

impl ProxySocks5 {
    pub fn new(
        username: Option<String>,
        password: Option<String>,
        bind_ip: Option<IpAddr>,
    ) -> Self {
        Self {
            username,
            password,
            bind_ip,
        }
    }

    /// 读取的信息, 并返回验证方法, 如果没有用户密码则表示无需认证
    pub async fn read_head_len<T>(
        &self,
        stream: &mut T,
        buffer: &mut BinaryMut,
    ) -> ProxyTypeResult<u8, T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let _ = ProxySocks5::read_len(stream, buffer, 2).await;
        if buffer.get_u8() != SOCKS5_VERSION {
            return Err(ProxyError::SizeNotMatch);
        }
        let len = buffer.get_u8() as usize;
        let _ = ProxySocks5::read_len(stream, buffer, len).await;
        let mut verify = 0;
        let chunk = buffer.chunk();
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

    /// 尝试是否验证成功
    pub async fn read_verify<T>(&self, stream: &mut T, buffer: &mut BinaryMut) -> ProxyResult<bool>
    where
        T: AsyncRead + Unpin,
    {
        let _ = ProxySocks5::read_len(stream, buffer, 2).await?;
        if buffer.get_u8() != 1 {
            return Err(ProxyError::ProtErr);
        }
        let user_len = buffer.get_u8() as usize;
        let _ = ProxySocks5::read_len(stream, buffer, user_len).await?;
        if let Some(v) = &self.username {
            if user_len == 0 || v.as_bytes() != &buffer.chunk()[0..user_len] {
                return Ok(false);
            }
            buffer.advance(user_len);
        }
        let _ = ProxySocks5::read_len(stream, buffer, 1).await?;
        let pass_len = buffer.get_u8() as usize;
        let _ = ProxySocks5::read_len(stream, buffer, pass_len).await?;
        if let Some(v) = &self.password {
            if pass_len == 0 || v.as_bytes() != &buffer.chunk()[0..pass_len] {
                return Ok(false);
            }
            buffer.advance(pass_len);
        }
        Ok(true)
    }

    /// 读取至少长度为size的大小的字节数, 如果足够则返回Ok(())
    pub async fn read_len<T>(stream: &mut T, buffer: &mut BinaryMut, size: usize) -> ProxyResult<()>
    where
        T: AsyncRead + Unpin,
    {
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

    /// +------+----------+----------+
    /// | ATYP | DST.ADDR | DST.PORT |
    /// +------+----------+----------+
    /// |  1   | Variable |    2     |
    /// +------+----------+----------+
    /// 读取通用地址格式，包含V4/V6/Doamin三种格式
    pub async fn read_addr<T>(stream: &mut T, buffer: &mut BinaryMut) -> ProxyResult<SocketAddr>
    where
        T: AsyncRead + Unpin,
    {
        let atyp = buffer.get_u8();
        let addr = match atyp {
            SOCKS5_ADDR_TYPE_IPV4 => {
                ProxySocks5::read_len(stream, buffer, 6).await?;
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
            SOCKS5_ADDR_TYPE_DOMAIN => {
                ProxySocks5::read_len(stream, buffer, 1).await?;
                let len = buffer.get_u8() as usize;
                ProxySocks5::read_len(stream, buffer, len + 2).await?;
                let name = String::from_utf8_lossy(&buffer.chunk()[0..len]).to_string();
                buffer.advance(len);
                let port = buffer.get_u16();
                let domain = format!("{}:{}", name, port);
                let mut addrs = domain.to_socket_addrs()?;
                addrs.next().unwrap()
            }
            SOCKS5_ADDR_TYPE_IPV6 => {
                ProxySocks5::read_len(stream, buffer, 18).await?;
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
            _ => return Err(ProxyError::ProtErr),
        };
        Ok(addr)
    }

    /// +------+----------+----------+
    /// | ATYP | DST.ADDR | DST.PORT |
    /// +------+----------+----------+
    /// |  1   | Variable |    2     |
    /// +------+----------+----------+
    /// 将地址转化成二进制流
    pub fn encode_socket_addr(buf: &mut BinaryMut, addr: &SocketAddr) -> ProxyResult<()> {
        let (addr_type, mut ip_oct, mut port) = match addr {
            SocketAddr::V4(sock) => (
                SOCKS5_ADDR_TYPE_IPV4,
                sock.ip().octets().to_vec(),
                sock.port().to_be_bytes().to_vec(),
            ),
            SocketAddr::V6(sock) => (
                SOCKS5_ADDR_TYPE_IPV6,
                sock.ip().octets().to_vec(),
                sock.port().to_be_bytes().to_vec(),
            ),
        };

        buf.put_u8(addr_type);
        buf.put_slice(&mut ip_oct);
        buf.put_slice(&mut port);
        Ok(())
    }

    /// +----+-----+-------+------+----------+----------+
    /// |VER | CMD |  RSV  | ATYP | DST.ADDR | DST.PORT |
    /// +----+-----+-------+------+----------+----------+
    /// | 1  |  1  | X'00' |  1   | Variable |    2     |
    /// +----+-----+-------+------+----------+----------+
    /// 解析request
    pub async fn tcp_read_request<T>(
        stream: &mut T,
        buffer: &mut BinaryMut,
    ) -> ProxyResult<(u8, SocketAddr)>
    where
        T: AsyncRead + Unpin,
    {
        let _ = ProxySocks5::read_len(stream, buffer, 4).await?;
        if buffer.get_u8() != SOCKS5_VERSION {
            return Err(ProxyError::ProtErr);
        }
        let sock = buffer.get_u8();
        if buffer.get_u8() != 0 {
            return Err(ProxyError::ProtErr);
        }
        let addr = Self::read_addr(stream, buffer).await?;
        return Ok((sock, addr));
    }

    pub async fn process<T>(
        &mut self,
        mut stream: T,
        buffer: Option<BinaryMut>,
    ) -> ProxyTypeResult<(), T>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        let mut buffer = buffer.unwrap_or(BinaryMut::new());
        let verify = match self.read_head_len(&mut stream, &mut buffer).await {
            Err(ProxyError::SizeNotMatch) => {
                return Err(ProxyError::Continue((Some(buffer), stream)));
            }
            Err(err) => {
                return Err(err);
            }
            Ok(result) => result,
        };

        let is_verify = {
            stream.write_all(&[SOCKS5_VERSION, verify]).await?;
            if verify == 0xFF {
                return Err(ProxyError::VerifyFail);
            }
            verify == 2
        };

        if is_verify {
            let succ = self
                .read_verify(&mut stream, &mut buffer)
                .await
                .map_err(|e| e.to_type::<T>())?;
            if !succ {
                stream.write_all(&[0x01_u8, 0x01]).await?;
                return Err(ProxyError::VerifyFail);
            } else {
                stream.write_all(&[0x01_u8, 0x00]).await?;
            }
        }

        let (sock, addr) = ProxySocks5::tcp_read_request(&mut stream, &mut buffer)
            .await
            .map_err(|e| e.to_type::<T>())?;
        match sock {
            SOCK_CONNECT => {
                let mut target = match TcpStream::connect(addr.clone()).await {
                    Ok(tcp) => {
                        stream.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
                        tcp
                    }
                    Err(err) => {
                        stream.write_all(&[5, 1, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
                        return Err(ProxyError::from(err));
                    }
                };

                let _ = copy_bidirectional(&mut stream, &mut target).await?;
            }
            // 不支持bind指令, 协议错误
            SOCK_BIND => {
                return Err(ProxyError::ProtNoSupport);
            }
            // 不支持bind指令, 协议错误
            SOCK_UDP => {
                if self.bind_ip.is_none() {
                    return Err(ProxyError::ProtNoSupport);
                }
                Self::udp_execute_assoc(
                    stream,
                    self.bind_ip
                        .unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                )
                .await
                .map_err(|e| e.to_type::<T>())?;
                return Ok(());
            }
            _ => {
                return Err(ProxyError::ProtErr);
            }
        }
        Ok(())
    }

    pub fn is_user_password(&self) -> bool {
        self.username.is_some() && self.password.is_some()
    }

    /// +----+-----+-------+------+----------+----------+
    /// |VER | REP |  RSV  | ATYP | BND.ADDR | BND.PORT |
    /// +----+-----+-------+------+----------+----------+
    /// | 1  |  1  | X'00' |  1   | Variable |    2     |
    /// +----+-----+-------+------+----------+----------+
    /// https://datatracker.ietf.org/doc/html/rfc1928#section-6
    pub async fn tcp_write_reply<T>(
        stream: &mut T,
        succ: bool,
        addr: SocketAddr,
    ) -> ProxyResult<()>
    where T: AsyncRead + AsyncWrite + Unpin {
        let mut buf = BinaryMut::with_capacity(100);
        buf.put_slice(&vec![SOCKS5_VERSION, if succ { 0 } else { 1 }, 0x00]);
        Self::encode_socket_addr(&mut buf, &addr)?;
        stream.write_all(&buf.chunk()).await?;
        Ok(())
    }

    /// UDP 关联请求用于在UDP中继进程内建立关联以处理UDP数据报。
    /// DST.ADDR和DST.PORT字段包含客户端期望用于发送UDP数据报的地址和端口。
    /// 服务器可以使用此信息来限制对关联的访问。如果客户端在UDP 关联请求时没有掌握此信息，
    /// 客户端必须使用端口号和地址都为零的地址。
    /// UDP关联会在随着的TCP连接终止时终止。
    /// 在UDP 关联请求的回复中，BND.PORT和BND.ADDR字段指示客户端必须发送UDP请求消息以进行中继的端口号/地址。
    /// https://datatracker.ietf.org/doc/html/rfc1928#section-7
    pub async fn udp_execute_assoc<T>(mut stream: T, bind_ip: IpAddr) -> ProxyResult<()>
    where T: AsyncRead + AsyncWrite + Unpin {
        let peer_sock = UdpSocket::bind("0.0.0.0:0").await?;
        let port = peer_sock.local_addr()?.port();
        ProxySocks5::tcp_write_reply(&mut stream, true, SocketAddr::new(bind_ip, port)).await?;
        Self::udp_transfer(stream, peer_sock).await?;
        Ok(())
    }

    ///   +----+------+------+----------+----------+----------+
    ///   |RSV | FRAG | ATYP | DST.ADDR | DST.PORT |   DATA   |
    ///   +----+------+------+----------+----------+----------+
    ///   | 2  |  1   |  1   | Variable |    2     | Variable |
    ///   +----+------+------+----------+----------+----------+
    ///  UDP和本地的通讯的头全部加上这个，因为中间隔了代理，需要转发到正确的地址上
    async fn udp_parse_request(buf: &mut BinaryMut) -> ProxyResult<(u8, SocketAddr)> {
        if buf.remaining() < 3 {
            return Err(ProxyError::ProtErr);
        }
        let _rsv = buf.get_u16();
        let flag = buf.get_u8();
        let array: Vec<u8> = vec![];
        let addr = ProxySocks5::read_addr(&mut &array[..], buf).await?;
        return Ok((flag, addr));
    }

    async fn upd_handle_tcp_block<T>(
        mut stream: T,
        mut receiver: Receiver<()>,
        sender: Sender<()>,
    ) -> ProxyResult<()>
    where T: AsyncRead + AsyncWrite + Unpin {
        let mut buf = [0u8; 100];
        loop {
            let n = tokio::select! {
                r = stream.read(&mut buf) => {
                    r?
                },
                _ = receiver.recv() => {
                    return Ok(());
                }
            };
            if n == 0 {
                let _ = sender.send(());
                return Ok(());
            }
        }
    }

    /// 处理收到客户端的消息, 解析发送到远程
    async fn udp_handle_request(
        inbound: &UdpSocket,
        outbound: &UdpSocket,
        mut receiver: Receiver<()>,
    ) -> ProxyResult<()> {
        let mut buf = BinaryMut::with_capacity(0x10000);
        loop {
            buf.clear();
            let (size, client_addr) = {
                let mut buf = ReadBuf::uninit(buf.chunk_mut());
                tokio::select! {
                    r = inbound.recv_buf_from(&mut buf) => {
                        r?
                    },
                    _ = receiver.recv() => {
                        return Ok(());
                    }
                }
            };
            unsafe {
                buf.advance_mut(size);
            }
            // 代理对内的端口只会跟客户端的通讯, 所以建立connect
            inbound.connect(client_addr).await?;

            let (flag, addr) = Self::udp_parse_request(&mut buf).await?;
            if flag != 0 {
                return Ok(());
            }

            outbound.send_to(buf.chunk(), addr).await?;
        }
    }

    /// 处理收到远程的消息, 添加头发送到客户端
    async fn udp_handle_response(
        inbound: &UdpSocket,
        outbound: &UdpSocket,
        mut receiver: Receiver<()>,
    ) -> ProxyResult<()> {
        let mut buf = BinaryMut::with_capacity(0x10000);
        loop {
            buf.clear();
            let (size, client_addr) = {
                let (size, client_addr) = {
                    let mut buf = ReadBuf::uninit(buf.chunk_mut());
                    tokio::select! {
                        r = outbound.recv_buf_from(&mut buf) => {
                            r?
                        },
                        _ = receiver.recv() => {
                            return Ok(());
                        }
                    }
                };
                (size, client_addr)
            };
            unsafe {
                buf.advance_mut(size);
            }

            let mut buffer = BinaryMut::with_capacity(100);
            buffer.put_slice(&[0, 0, 0]);
            ProxySocks5::encode_socket_addr(&mut buffer, &client_addr)?;
            buffer.put_slice(buf.chunk());

            // 因为已经建立了绑定, 所以直接发送
            inbound.send(buffer.chunk()).await?;
        }
    }

    async fn udp_transfer<T>(stream: T, inbound: UdpSocket) -> ProxyResult<()>
    where T: AsyncRead + AsyncWrite + Unpin {
        let outbound = UdpSocket::bind("0.0.0.0:0").await?;
        // 使tcp断开的时候通知udp结束关联,结束处理函数
        let (sender, receiver) = channel::<()>(1);
        let req_fut = Self::udp_handle_request(&inbound, &outbound, receiver);
        let res_fut = Self::udp_handle_response(&inbound, &outbound, sender.subscribe());
        let tcp_fut = Self::upd_handle_tcp_block(stream, sender.subscribe(), sender.clone());
        match try_join!(tcp_fut, req_fut, res_fut) {
            Ok(_) => {}
            Err(error) => {
                // 发生错误时不确定是哪个处理函数出错, 通知其它的停止
                let _ = sender.send(());
                return Err(error);
            }
        }
        Ok(())
    }
}
