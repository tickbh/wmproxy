// Copyright 2022 - 2023 Wenmeng See the COPYRIGHT
// file at the top-level directory of this distribution.
// 
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
// 
// Author: tickbh
// -----
// Created Date: 2023/09/22 10:28:28

use webparse::{Buf, BufMut};

use crate::{
    prot::{ProtFlag, ProtKind},
    ProxyResult,
};

use super::ProtFrameHeader;

/// 新的Socket连接请求, 
/// 接收方创建一个虚拟链接来对应该Socket的读取写入
#[derive(Debug)]
#[allow(dead_code)]
pub struct ProtCreate {
    server_id: u32,
    sock_map: u32,
    mode: u8,
    domain: Option<String>,
}

impl ProtCreate {
    pub fn new(server_id: u32,sock_map: u32, domain: Option<String>) -> Self {
        Self {
            server_id,
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
            server_id: header.server_id(),
            sock_map: header.sock_map(),
            mode: 0,
            domain,
        })
    }

    pub fn encode<B: Buf + BufMut>(self, buf: &mut B) -> ProxyResult<usize> {
        let mut head = ProtFrameHeader::new(ProtKind::Create, ProtFlag::zero(), self.sock_map, self.server_id);
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

    pub fn domain(&self) -> &Option<String> {
        &self.domain
    }
}
