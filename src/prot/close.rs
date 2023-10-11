use webparse::{Buf, BufMut};

use crate::{
    prot::{ProtFlag, ProtKind},
    ProxyResult,
};

use super::{ProtFrameHeader, read_short_string, write_short_string};

/// 旧的Socket连接关闭, 接收到则关闭掉当前的连接
#[derive(Debug)]
pub struct ProtClose {
    sock_map: u32,
    reason: String,
}

impl ProtClose {
    pub fn new(sock_map: u32) -> ProtClose {
        ProtClose { sock_map, reason: String::new() }
    }

    pub fn new_by_reason(sock_map: u32, reason: String) -> ProtClose {
        ProtClose { sock_map, reason }
    }

    pub fn parse<T: Buf>(header: ProtFrameHeader, mut buf: T) -> ProxyResult<ProtClose> {
        let reason = read_short_string(&mut buf)?;
        Ok(ProtClose {
            sock_map: header.sock_map(),
            reason,
        })
    }

    pub fn encode<B: Buf + BufMut>(self, buf: &mut B) -> ProxyResult<usize> {
        let mut head = ProtFrameHeader::new(ProtKind::Close, ProtFlag::zero(), self.sock_map);
        head.length = self.reason.as_bytes().len() as u32 + 1;
        let mut size = 0;
        size += head.encode(buf)?;
        size += write_short_string(buf, &self.reason)?;
        Ok(size)
    }

    pub fn sock_map(&self) -> u32 {
        self.sock_map
    }

    pub fn reason(&self) -> &String {
        &self.reason
    }
}
