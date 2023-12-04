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
// Created Date: 2023/10/07 09:40:42

use webparse::{Buf, BufMut, BinaryMut, must_have};

use crate::{
    prot::{ProtFlag, ProtKind},
    ProxyResult, MappingConfig, HeaderOper, ConfigHeader,
};

use super::{ProtFrameHeader, read_short_string, write_short_string};

/// 新的Socket连接请求, 
/// 接收方创建一个虚拟链接来对应该Socket的读取写入
#[derive(Debug)]
pub struct ProtMapping {
    sock_map: u32,
    pub mappings: Vec<MappingConfig>,
}

impl ProtMapping {
    pub fn new(sock_map: u32, mappings: Vec<MappingConfig>) -> Self {
        Self {
            sock_map,
            mappings,
        }
    }

    pub fn parse<T: Buf>(header: ProtFrameHeader, mut buf: T) -> ProxyResult<ProtMapping> {
        must_have!(buf, 2)?;
        let len = buf.get_u16() as usize;
        let mut mappings = vec![];
        
        for _ in 0..len {
            let name = read_short_string(&mut buf)?;
            let mode = read_short_string(&mut buf)?;
            let domain = read_short_string(&mut buf)?;
            let mut headers = vec![];
            must_have!(buf, 2)?;
            let len = buf.get_u16();
            for _ in 0 .. len {
                must_have!(buf, 2)?;
                let is_proxy = buf.get_u8() != 0;
                let oper = HeaderOper::from_u8(buf.get_u8());
                let key = read_short_string(&mut buf)?;
                let val = read_short_string(&mut buf)?;
                headers.push(ConfigHeader::new(oper, is_proxy, key, val));
            }
            mappings.push(MappingConfig::new(name, mode, domain, headers));
        }
        Ok(ProtMapping {
            sock_map: header.sock_map(),
            mappings,
        })
    }

    pub fn encode<B: Buf + BufMut>(self, buf: &mut B) -> ProxyResult<usize> {
        let mut head = ProtFrameHeader::new(ProtKind::Mapping, ProtFlag::zero(), self.sock_map);

        let mut cache_buf = BinaryMut::with_capacity(100);
        cache_buf.put_u16(self.mappings.len() as u16);
        for m in self.mappings {
            write_short_string(&mut cache_buf, &m.name)?;
            write_short_string(&mut cache_buf, &m.mode)?;
            write_short_string(&mut cache_buf, &m.domain)?;
            cache_buf.put_u16(m.headers.len() as u16);
            for value in m.headers {
                cache_buf.put_u8(value.is_proxy as u8);
                cache_buf.put_u8(value.oper.to_u8());
                write_short_string(&mut cache_buf, &value.key)?;
                write_short_string(&mut cache_buf, &value.val)?;
            }
        }
        head.length = cache_buf.remaining() as u32;
        let mut size = 0;
        size += head.encode(buf)?;
        size += buf.put_slice(&cache_buf.chunk());
        Ok(size)
    }

    pub fn sock_map(&self) -> u32 {
        self.sock_map
    }

    pub fn mappings(&self) -> &Vec<MappingConfig> {
        &self.mappings
    }


    pub fn into_mappings(self) -> Vec<MappingConfig> {
        self.mappings
    }
}
