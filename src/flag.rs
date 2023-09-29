use bitflags::bitflags;
use serde::{Serialize, Deserialize};

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct Flag: u8 {
        /// 使用HTTP代理类型
        const HTTP = 0x1;
        /// 使用HTTPS代理类型
        const HTTPS = 0x2;
        /// 使用SOCKS5代理类型
        const SOCKS5 = 0x4;
        /// 纯TCP转发
        const TCP = 0x8;
        /// 纯UDP转发
        const UDP = 0x16;
    }
}


impl Serialize for Flag {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer {
        serializer.serialize_u8(self.bits())
    }
}

impl<'a> Deserialize<'a> for Flag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a> {
        let v = u8::deserialize(deserializer)?;
        Ok(Flag::from_bits(v).unwrap_or(Flag::HTTP))
    }
}

