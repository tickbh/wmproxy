use webparse::{Binary, Buf, BufMut, Serialize};

use crate::{
    prot::{ProtFlag, ProtKind},
    ProxyResult,
};

use super::{read_short_string, write_short_string, ProtFrameHeader};

/// Socket的数据消息包
#[derive(Debug)]
pub struct ProtToken {
    username: String,
    password: String,
}

impl ProtToken {
    pub fn new(username: String, password: String) -> ProtToken {
        Self { username, password }
    }

    pub fn parse<T: Buf>(_header: ProtFrameHeader, mut buf: T) -> ProxyResult<ProtToken> {
        let username = read_short_string(&mut buf)?;
        let password = read_short_string(&mut buf)?;
        Ok(Self { username, password })
    }

    pub fn encode<B: Buf + BufMut>(self, buf: &mut B) -> ProxyResult<usize> {
        let mut head = ProtFrameHeader::new(ProtKind::Token, ProtFlag::zero(), 0);
        head.length = self.username.as_bytes().len() as u32 + 1 + self.password.as_bytes().len() as u32 + 1;
        let mut size = 0;
        size += head.encode(buf)?;
        size += write_short_string(buf, &self.username)?;
        size += write_short_string(buf, &self.password)?;
        Ok(size)
    }

    pub fn username(&self) -> &String {
        &self.username
    }
    
    pub fn password(&self) -> &String {
        &self.password
    }

    pub fn is_check_succ(&self, username: &Option<String>, password: &Option<String>) -> bool {
        if username.is_some() && username.as_ref().unwrap() != &self.username {
            return false;
        }
        if password.is_some() && password.as_ref().unwrap() != &self.password {
            return false;
        }
        return true
    }

    pub fn sock_map(&self) -> u32 {
        0
    }
}
