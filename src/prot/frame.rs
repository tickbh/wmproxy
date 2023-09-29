
use webparse::{Buf, http2::frame::{read_u24, encode_u24}, BufMut, Binary};

use crate::ProxyResult;

use super::{ProtCreate, ProtClose, ProtData, ProtFlag, ProtKind, ProtMapping};


#[derive(Debug)]
pub struct ProtFrameHeader {
    /// 包体的长度, 3个字节, 最大为16m
    pub length: u32,
    /// 包体的类型, 如Create, Data等
    kind: ProtKind,
    /// 包体的标识, 如是否为响应包等
    flag: ProtFlag,
    /// 3个字节, socket在内存中相应的句柄, 客户端发起为单数, 服务端发起为双数
    sock_map: u32,
}

#[derive(Debug)]
pub enum ProtFrame {
    /// 收到新的Socket连接
    Create(ProtCreate),
    /// 收到旧的Socket连接关闭
    Close(ProtClose),
    /// 收到Socket的相关数据
    Data(ProtData),
    /// 收到内网映射的相关消息
    Mapping(ProtMapping),
}

impl ProtFrameHeader {
    pub const FRAME_HEADER_BYTES: usize = 8;

    pub fn new(kind: ProtKind, flag: ProtFlag, sock_map: u32) -> ProtFrameHeader {
        ProtFrameHeader {
            length: 0,
            kind,
            flag,
            sock_map,
        }
    }
    
    pub fn sock_map(&self) -> u32 {
        self.sock_map
    }

    pub fn flag(&self) -> ProtFlag {
        self.flag
    }

    #[inline]
    pub fn parse<T: Buf>(buffer: &mut T) -> ProxyResult<ProtFrameHeader> {
        if buffer.remaining() < Self::FRAME_HEADER_BYTES {
            return Err(crate::ProxyError::TooShort);
        }
        let length = read_u24(buffer);
        Self::parse_by_len(buffer, length)
    }

    #[inline]
    pub fn parse_by_len<T: Buf>(buffer: &mut T, length: u32) -> ProxyResult<ProtFrameHeader> {
        if buffer.remaining() < Self::FRAME_HEADER_BYTES - 3 {
            return Err(crate::ProxyError::TooShort);
        }
        let kind = buffer.get_u8();
        let flag = buffer.get_u8();
        let sock_map = read_u24(buffer);
        Ok(ProtFrameHeader {
            length,
            kind: ProtKind::new(kind),
            flag: ProtFlag::new(flag),
            sock_map,
        })
    }


    pub fn encode<B: Buf + BufMut>(&self, buffer: &mut B) -> ProxyResult<usize> {
        let mut size = 0;
        size += encode_u24(buffer, self.length);
        size += buffer.put_u8(self.kind.encode());
        size += buffer.put_u8(self.flag.bits());
        size += encode_u24(buffer, self.sock_map);
        Ok(size)
    }

}

impl ProtFrame {
    /// 把字节流转化成数据对象
    pub fn parse<T: Buf>(
        header: ProtFrameHeader,
        buf: T,
    ) -> ProxyResult<ProtFrame> {
        let v = match header.kind {
            ProtKind::Data => ProtFrame::Data(ProtData::parse(header, buf)?) ,
            ProtKind::Create => ProtFrame::Create(ProtCreate::parse(header, buf)?),
            ProtKind::Close => ProtFrame::Close(ProtClose::parse(header, buf)?),
            ProtKind::Mapping => ProtFrame::Mapping(ProtMapping::parse(header, buf)?),
            ProtKind::Unregistered => todo!(),
        };
        Ok(v)
    }

    /// 把数据对象转化成字节流
    pub fn encode<B: Buf + BufMut>(
        self,
        buf: &mut B,
    ) -> ProxyResult<usize> {
        let size = match self {
            ProtFrame::Data(s) => s.encode(buf)?,
            ProtFrame::Create(s) => s.encode(buf)?,
            ProtFrame::Close(s) => s.encode(buf)?,
            ProtFrame::Mapping(s) => s.encode(buf)?,
        };
        Ok(size)
    }

    pub fn new_create(sock_map: u32, domain: Option<String>) -> Self {
        Self::Create(ProtCreate::new(sock_map, domain))
    }

    pub fn new_close(sock_map: u32) -> Self {
        Self::Close(ProtClose::new(sock_map))
    }

    pub fn new_data(sock_map: u32, data: Binary) -> Self {
        Self::Data(ProtData::new(sock_map, data))
    }

    pub fn is_create(&self) -> bool {
        match self {
            ProtFrame::Create(_) => true,
            _ => false
        }
    }

    pub fn is_close(&self) -> bool {
        match self {
            ProtFrame::Close(_) => true,
            _ => false
        }
    }

    pub fn is_data(&self) -> bool {
        match self {
            ProtFrame::Data(_) => true,
            _ => false
        }
    }

    pub fn sock_map(&self) -> u32 {
        match self {
            ProtFrame::Data(s) => s.sock_map(),
            ProtFrame::Create(s) => s.sock_map(),
            ProtFrame::Close(s) => s.sock_map(),
            ProtFrame::Mapping(s) => s.sock_map(),
        }
    }

}