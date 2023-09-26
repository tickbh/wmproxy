use webparse::{Buf, BufMut};

use crate::{
    prot::{ProtFlag, ProtKind},
    ProxyResult,
};

use super::ProtFrameHeader;

#[derive(Debug)]
pub struct ProtCreate {
    sock_map: u32,
    mode: u8,
    domain: Option<String>,
}

impl ProtCreate {
    pub fn new(sock_map: u32) -> Self {
        Self {
            sock_map,
            mode: 0,
            domain: None,
        }
    }
    pub fn parse<T: Buf>(header: ProtFrameHeader, mut buf: T) -> ProxyResult<ProtCreate> {
        let mode = buf.get_u8();
        Ok(ProtCreate {
            sock_map: header.sock_map(),
            mode,
            domain: None,
        })
    }

    pub fn encode<B: Buf + BufMut>(self, buf: &mut B) -> ProxyResult<usize> {
        let mut head = ProtFrameHeader::new(ProtKind::Create, ProtFlag::zero(), self.sock_map);
        head.length = 1;
        let mut size = 0;
        size += head.encode(buf)?;
        size += buf.put_u8(self.mode);
        Ok(size)
    }

    pub fn sock_map(&self) -> u32 {
        self.sock_map
    }
}
