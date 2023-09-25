use webparse::{Buf, BufMut};

use crate::{
    prot::{ProtFlag, ProtKind},
    ProxyResult,
};

use super::ProtFrameHeader;

pub struct ProtClose {
    sock_map: u32,
}

impl ProtClose {
    pub fn new(sock_map: u32) -> ProtClose {
        ProtClose { sock_map }
    }

    pub fn parse<T: Buf>(header: ProtFrameHeader, mut buf: T) -> ProxyResult<ProtClose> {
        let mode = buf.get_u8();
        Ok(ProtClose {
            sock_map: header.sock_map(),
        })
    }

    pub fn encode<B: Buf + BufMut>(self, buf: &mut B) -> ProxyResult<usize> {
        let mut head = ProtFrameHeader::new(ProtKind::Create, ProtFlag::zero(), self.sock_map);
        head.length = 1;
        let mut size = 0;
        size += head.encode(buf)?;
        Ok(size)
    }
}
