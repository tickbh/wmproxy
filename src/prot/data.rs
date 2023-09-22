use webparse::{Binary, Buf, BufMut, Serialize};

use crate::{ProxyResult, prot::{ProtKind, ProtFlag}};

use super::ProtFrameHeader;

pub struct ProtData {
    sock_map: u32,
    data: Binary,
}


impl ProtData {
    pub fn parse<T: Buf>(
        header: ProtFrameHeader,
        buf: T,
    ) -> ProxyResult<ProtData> {
        Ok(Self {
            sock_map: header.sock_map(),
            data: buf.into_binary()
        })
    }

    pub fn encode<B: Buf + BufMut>(
        mut self,
        buf: &mut B,
    ) -> ProxyResult<usize> {

        log::trace!("encoding Data; len={}", self.data.remaining());
        let mut head = ProtFrameHeader::new(ProtKind::Data, ProtFlag::zero(), self.sock_map);
        head.length = self.data.remaining() as u32;
        let mut size = 0;
        size += head.encode(buf)?;
        size += self.data.serialize(buf)?;
        Ok(size)
    }
}