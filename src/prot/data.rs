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
// Created Date: 2023/09/22 10:28:41

use webparse::{Binary, Buf, BufMut, Serialize};

use crate::{
    prot::{ProtFlag, ProtKind},
    ProxyResult,
};

use super::ProtFrameHeader;

/// Socket的数据消息包
#[derive(Debug)]
pub struct ProtData {
    sock_map: u32,
    data: Binary,
}

impl ProtData {
    pub fn new(sock_map: u32, data: Binary) -> ProtData {
        Self { sock_map, data }
    }

    pub fn parse<T: Buf>(header: ProtFrameHeader, buf: T) -> ProxyResult<ProtData> {
        Ok(Self {
            sock_map: header.sock_map(),
            data: buf.into_binary(),
        })
    }

    pub fn encode<B: Buf + BufMut>(mut self, buf: &mut B) -> ProxyResult<usize> {
        log::trace!("encoding Data; len={}", self.data.remaining());
        let mut head = ProtFrameHeader::new(ProtKind::Data, ProtFlag::zero(), self.sock_map);
        head.length = self.data.remaining() as u32;
        let mut size = 0;
        size += head.encode(buf)?;
        size += self.data.serialize(buf)?;
        Ok(size)
    }

    pub fn data(&self) -> &Binary {
        &self.data
    }

    pub fn sock_map(&self) -> u32 {
        self.sock_map
    }
}
