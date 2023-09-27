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
    pub fn new(sock_map: u32, domain: Option<String>) -> Self {
        Self {
            sock_map,
            mode: 0,
            domain,
        }
    }

    pub fn parse<T: Buf>(header: ProtFrameHeader, mut buf: T) -> ProxyResult<ProtCreate> {
        let length = buf.get_u8() as usize;
        let mut domain = None;
        if length > buf.remaining() {
            return Err(crate::ProxyError::TooShort);
        }
        if length > 0 {
            let data = &buf.chunk()[..length];
            domain = Some(String::from_utf8_lossy(data).to_string());
        }
        Ok(ProtCreate {
            sock_map: header.sock_map(),
            mode: 0,
            domain,
        })
    }

    pub fn encode<B: Buf + BufMut>(self, buf: &mut B) -> ProxyResult<usize> {
        let mut head = ProtFrameHeader::new(ProtKind::Create, ProtFlag::zero(), self.sock_map);
        let domain_len = self.domain.as_ref().map(|s| s.as_bytes().len() as u32).unwrap_or(0);
        head.length = 1 + domain_len;
        let mut size = 0;
        size += head.encode(buf)?;
        size += buf.put_u8(domain_len as u8);
        if let Some(d) = &self.domain {
            size += buf.put_slice(d.as_bytes());
        }
        Ok(size)
    }

    pub fn sock_map(&self) -> u32 {
        self.sock_map
    }
}
