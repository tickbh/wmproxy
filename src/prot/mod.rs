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
// Created Date: 2023/09/22 09:59:56


mod flag;
mod create;
mod close;
mod data;
mod frame;
mod kind;
mod mapping;
mod token;

pub use flag::ProtFlag;
pub use kind::ProtKind;
pub use create::ProtCreate;
pub use close::ProtClose;
pub use data::ProtData;
pub use mapping::ProtMapping;
pub use token::ProtToken;
pub use frame::{ProtFrame, ProtFrameHeader};

use webparse::{Buf, BufMut};

use crate::ProxyResult;

fn read_short_string<T: Buf>(buf: &mut T) -> ProxyResult<String> {
    if buf.remaining() < 1 {
        return Err(crate::ProxyError::TooShort);
    }
    let len = buf.get_u8() as usize;
    if buf.remaining() < len {
        return Err(crate::ProxyError::TooShort);
    } else if len == 0 {
        return Ok(String::new());
    }
    let s = String::from_utf8_lossy(&buf.chunk()[0..len]).to_string();
    buf.advance(len);
    Ok(s)
}

fn write_short_string<T: Buf + BufMut>(buf: &mut T, val: &str) -> ProxyResult<usize> {
    let bytes = val.as_bytes();
    if bytes.len() > 255 {
        return Err(crate::ProxyError::TooShort);
    }
    let mut size = 0;
    size += buf.put_u8(bytes.len() as u8);
    size += buf.put_slice(bytes);
    Ok(size)
}