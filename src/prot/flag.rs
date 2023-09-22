use bitflags::bitflags;

bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct ProtFlag: u8 {
        /// 返回的消息
        const ACK = 0x1;
        /// 创建消息
        const CREATE = 0x2;
        /// 关闭消息
        const CLOSE = 0x4;
        /// 数据消息
        const DATA = 0x8;
    }
}

impl ProtFlag {
    pub fn zero() -> ProtFlag {
        ProtFlag::default()
    }

    pub fn new(data: u8) -> ProtFlag {
        match ProtFlag::from_bits(data) {
            Some(v) => v,
            None => ProtFlag::default(),
        }
    }

    pub fn load(mut flag: ProtFlag) -> ProtFlag {
        flag.set(ProtFlag::ACK, true);
        flag
    }

    pub fn ack() -> ProtFlag {
        ProtFlag::ACK
    }

    pub fn is_ack(&self) -> bool {
        self.contains(ProtFlag::ACK)
    }

    pub fn create() -> ProtFlag {
        ProtFlag::CREATE
    }
    pub fn is_create(&self) -> bool {
        self.contains(ProtFlag::CREATE)
    }

    pub fn close() -> ProtFlag {
        ProtFlag::CLOSE
    }

    pub fn is_close(&self) -> bool {
        self.contains(ProtFlag::CLOSE)
    }

    pub fn data() -> ProtFlag {
        ProtFlag::DATA
    }

    pub fn is_data(&self) -> bool {
        self.contains(ProtFlag::DATA)
    }

    pub fn kind(&self) -> Self {
        let mut new = self.clone();
        new.set(ProtFlag::ACK, false);
        new
    }
}

impl Default for ProtFlag {
    fn default() -> Self {
        Self(Default::default())
    }
}
