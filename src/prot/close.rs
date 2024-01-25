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
// Created Date: 2023/09/22 10:28:38

use webparse::{Buf, BufMut};

use crate::{
    prot::{ProtFlag, ProtKind},
    ProxyResult,
};

use super::{ProtFrameHeader, read_short_string, write_short_string};

/// 旧的Socket连接关闭, 接收到则关闭掉当前的连接
#[derive(Debug)]
pub struct ProtClose {
    server_id: u32,
    sock_map: u32,
    reason: String,
}

impl ProtClose {
    pub fn new(server_id: u32, sock_map: u32) -> ProtClose {
        ProtClose { server_id, sock_map, reason: String::new() }
    }

    pub fn new_by_reason(server_id: u32, sock_map: u32, reason: String) -> ProtClose {
        ProtClose { server_id, sock_map, reason }
    }

    pub fn parse<T: Buf>(header: ProtFrameHeader, mut buf: T) -> ProxyResult<ProtClose> {
        let reason = read_short_string(&mut buf)?;
        Ok(ProtClose {
            server_id: header.server_id(),
            sock_map: header.sock_map(),
            reason,
        })
    }

    pub fn encode<B: Buf + BufMut>(self, buf: &mut B) -> ProxyResult<usize> {
        let mut head = ProtFrameHeader::new(ProtKind::Close, ProtFlag::zero(), self.sock_map, self.server_id);
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
