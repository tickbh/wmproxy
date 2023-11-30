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
// Created Date: 2023/09/22 11:05:30

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProtKind {
    Data = 0,
    Create = 1,
    Close = 2,
    Mapping = 3,
    Token = 4,
    Unregistered
}

impl ProtKind {
    pub fn new(byte: u8) -> ProtKind {
        return match byte {
            0 => ProtKind::Data,
            1 => ProtKind::Create,
            2 => ProtKind::Close,
            3 => ProtKind::Mapping,
            4 => ProtKind::Token,
            _ => ProtKind::Unregistered
        }
    }

    pub fn encode(&self) -> u8 {
        match *self {
            ProtKind::Data => 0,
            ProtKind::Create => 1,
            ProtKind::Close => 2,
            ProtKind::Mapping => 3,
            ProtKind::Token => 4,
            ProtKind::Unregistered => 255
        }
    }
}
