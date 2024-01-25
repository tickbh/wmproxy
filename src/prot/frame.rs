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
// Created Date: 2023/09/22 10:30:10


use tokio_util::bytes::buf;
use webparse::{Buf, http2::frame::{read_u24, encode_u24}, BufMut};

use crate::{ProxyResult, MappingConfig};

use super::{ProtCreate, ProtClose, ProtData, ProtFlag, ProtKind, ProtMapping, ProtToken};


#[derive(Debug)]
pub struct ProtFrameHeader {
    /// 包体的长度, 3个字节, 最大为16m
    pub length: u32,
    /// 包体的类型, 如Create, Data等
    kind: ProtKind,
    /// 包体的标识, 如是否为响应包等
    flag: ProtFlag,
    /// 3个字节, socket在内存中相应的句柄, 客户端发起为单数, 服务端发起为双数
    sock_map: u32,
    /// 服务器的id
    server_id: u32,
}

#[derive(Debug)]
pub enum ProtFrame {
    /// 收到新的Socket连接
    Create(ProtCreate),
    /// 收到旧的Socket连接关闭
    Close(ProtClose),
    /// 收到Socket的相关数据
    Data(ProtData),
    /// 收到Token的相关数据
    Token(ProtToken),
    /// 收到内网映射的相关消息
    Mapping(ProtMapping),
}

impl ProtFrameHeader {
    pub const FRAME_HEADER_BYTES: usize = 12;

    pub fn new(kind: ProtKind, flag: ProtFlag, sock_map: u32, server_id: u32) -> ProtFrameHeader {
        ProtFrameHeader {
            length: 0,
            kind,
            flag,
            sock_map,
            server_id,
        }
    }
    
    pub fn server_id(&self) -> u32 {
        self.server_id
    }

    pub fn sock_map(&self) -> u32 {
        self.sock_map
    }

    pub fn flag(&self) -> ProtFlag {
        self.flag
    }

    #[inline]
    pub fn parse<T: Buf>(buffer: &mut T) -> ProxyResult<ProtFrameHeader> {
        if buffer.remaining() < Self::FRAME_HEADER_BYTES {
            return Err(crate::ProxyError::TooShort);
        }
        let length = read_u24(buffer);
        Self::parse_by_len(buffer, length)
    }

    #[inline]
    pub fn parse_by_len<T: Buf>(buffer: &mut T, length: u32) -> ProxyResult<ProtFrameHeader> {
        if buffer.remaining() < Self::FRAME_HEADER_BYTES - 3 {
            return Err(crate::ProxyError::TooShort);
        }
        let kind = buffer.get_u8();
        let flag = buffer.get_u8();
        let sock_map = read_u24(buffer);
        let server_id = buffer.get_u32();
        Ok(ProtFrameHeader {
            length,
            kind: ProtKind::new(kind),
            flag: ProtFlag::new(flag),
            sock_map,
            server_id,
        })
    }


    pub fn encode<B: Buf + BufMut>(&self, buffer: &mut B) -> ProxyResult<usize> {
        let mut size = 0;
        size += encode_u24(buffer, self.length);
        size += buffer.put_u8(self.kind.encode());
        size += buffer.put_u8(self.flag.bits());
        size += encode_u24(buffer, self.sock_map);
        size += buffer.put_u32(self.server_id);
        Ok(size)
    }

}

impl ProtFrame {
    /// 把字节流转化成数据对象
    pub fn parse<T: Buf>(
        header: ProtFrameHeader,
        buf: T,
    ) -> ProxyResult<ProtFrame> {
        let v = match header.kind {
            ProtKind::Data => ProtFrame::Data(ProtData::parse(header, buf)?) ,
            ProtKind::Create => ProtFrame::Create(ProtCreate::parse(header, buf)?),
            ProtKind::Close => ProtFrame::Close(ProtClose::parse(header, buf)?),
            ProtKind::Mapping => ProtFrame::Mapping(ProtMapping::parse(header, buf)?),
            ProtKind::Token => ProtFrame::Token(ProtToken::parse(header, buf)?),
            ProtKind::Unregistered => todo!(),
        };
        Ok(v)
    }

    /// 把数据对象转化成字节流
    pub fn encode<B: Buf + BufMut>(
        self,
        buf: &mut B,
    ) -> ProxyResult<usize> {
        let size = match self {
            ProtFrame::Data(s) => s.encode(buf)?,
            ProtFrame::Create(s) => s.encode(buf)?,
            ProtFrame::Close(s) => s.encode(buf)?,
            ProtFrame::Mapping(s) => s.encode(buf)?,
            ProtFrame::Token(s) => s.encode(buf)?,
        };
        Ok(size)
    }

    pub fn new_create(server_id: u32, sock_map: u32, domain: Option<String>) -> Self {
        Self::Create(ProtCreate::new(server_id, sock_map, domain))
    }

    pub fn new_close(sock_map: u32) -> Self {
        Self::Close(ProtClose::new(sock_map))
    }

    pub fn new_close_reason(sock_map: u32, reason: String) -> Self {
        Self::Close(ProtClose::new_by_reason(sock_map, reason))
    }

    pub fn new_data(sock_map: u32, data: Vec<u8>) -> Self {
        Self::Data(ProtData::new(sock_map, data))
    }

    pub fn new_mapping(sock_map: u32, mappings: Vec<MappingConfig>) -> Self {
        Self::Mapping(ProtMapping::new(sock_map, mappings))
    }

    pub fn new_token(username: String, password: String) -> Self {
        Self::Token(ProtToken::new(username, password))
    }

    pub fn is_create(&self) -> bool {
        match self {
            ProtFrame::Create(_) => true,
            _ => false
        }
    }

    pub fn is_close(&self) -> bool {
        match self {
            ProtFrame::Close(_) => true,
            _ => false
        }
    }

    pub fn is_data(&self) -> bool {
        match self {
            ProtFrame::Data(_) => true,
            _ => false
        }
    }

    pub fn is_mapping(&self) -> bool {
        match self {
            ProtFrame::Mapping(_) => true,
            _ => false
        }
    }

    pub fn sock_map(&self) -> u32 {
        match self {
            ProtFrame::Data(s) => s.sock_map(),
            ProtFrame::Create(s) => s.sock_map(),
            ProtFrame::Close(s) => s.sock_map(),
            ProtFrame::Mapping(s) => s.sock_map(),
            ProtFrame::Token(s) => s.sock_map(),
        }
    }

}