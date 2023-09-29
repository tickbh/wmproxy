#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProtKind {
    Data = 0,
    Create = 1,
    Close = 2,
    Mapping = 3,
    Unregistered
}

impl ProtKind {
    pub fn new(byte: u8) -> ProtKind {
        return match byte {
            0 => ProtKind::Data,
            1 => ProtKind::Create,
            2 => ProtKind::Close,
            3 => ProtKind::Mapping,
            _ => ProtKind::Unregistered
        }
    }

    pub fn encode(&self) -> u8 {
        match *self {
            ProtKind::Data => 0,
            ProtKind::Create => 1,
            ProtKind::Close => 2,
            ProtKind::Mapping => 3,
            ProtKind::Unregistered => 255
        }
    }
}
