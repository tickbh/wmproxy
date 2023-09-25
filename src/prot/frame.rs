use webparse::{Buf, http2::frame::{read_u24, encode_u24}, BufMut};

use crate::ProxyResult;

use super::{ProtCreate, ProtClose, ProtData, ProtFlag, ProtKind};

pub const FRAME_HEADER_BYTES: usize = 8;

pub struct ProtFrameHeader {
    pub length: u32,
    kind: ProtKind,
    flag: ProtFlag,
    sock_map: u32,
}

pub enum ProtFrame {
    Create(ProtCreate),
    Close(ProtClose),
    Data(ProtData),
}

impl ProtFrameHeader {
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
        if buffer.remaining() < FRAME_HEADER_BYTES {
            return Err(crate::ProxyError::TooShort);
        }
        let length = read_u24(buffer);
        Self::parse_by_len(buffer, length)
    }

    #[inline]
    pub fn parse_by_len<T: Buf>(buffer: &mut T, length: u32) -> ProxyResult<ProtFrameHeader> {
        if buffer.remaining() < FRAME_HEADER_BYTES - 3 {
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
        size += buffer.put_u8(self.flag.bits());
        size += buffer.put_u32(self.sock_map);
        Ok(size)
    }

}

impl ProtFrame {
    pub fn parse<T: Buf>(
        header: ProtFrameHeader,
        buf: T,
    ) -> ProxyResult<ProtFrame> {
        let v = match header.kind {
            ProtKind::Data => ProtFrame::Data(ProtData::parse(header, buf)?) ,
            ProtKind::Create => ProtFrame::Create(ProtCreate::parse(header, buf)?),
            ProtKind::Close => ProtFrame::Close(ProtClose::parse(header, buf)?),
            ProtKind::Unregistered => todo!(),
        };
        Ok(v)
    }

    pub fn encode<B: Buf + BufMut>(
        self,
        buf: &mut B,
    ) -> ProxyResult<usize> {
        let size = match self {
            ProtFrame::Data(s) => s.encode(buf)?,
            ProtFrame::Create(s) => s.encode(buf)?,
            ProtFrame::Close(s) => s.encode(buf)?,
        };
        Ok(size)
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
        }
    }

}