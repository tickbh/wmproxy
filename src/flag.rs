use bitflags::bitflags;

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

