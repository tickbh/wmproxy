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
    sock_map: u64,
    reason: String,
}

impl ProtClose {
    pub fn new(sock_map: u64) -> ProtClose {
        ProtClose { sock_map, reason: String::new() }
    }

    pub fn new_by_reason(sock_map: u64, reason: String) -> ProtClose {
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

    pub fn sock_map(&self) -> u64 {
        self.sock_map
    }

    pub fn reason(&self) -> &String {
        &self.reason
    }
}
