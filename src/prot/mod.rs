
mod flag;
mod create;
mod close;
mod data;
mod frame;
mod kind;

pub use flag::ProtFlag;
pub use kind::ProtKind;
pub use create::ProtCreate;
pub use close::ProtClose;
pub use data::ProtData;
pub use frame::{ProtFrame, ProtFrameHeader};

use webparse::{Buf, BufMut};

use crate::ProxyResult;

fn read_string<T: Buf>(buf: &mut T) -> ProxyResult<String> {
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

fn write_string<T: Buf + BufMut>(buf: &mut T, val: &str) -> ProxyResult<usize> {
    let bytes = val.as_bytes();
    if bytes.len() > 255 {
        return Err(crate::ProxyError::TooShort);
    }
    let mut size = 0;
    size += buf.put_u8(bytes.len() as u8);
    size += buf.put_slice(bytes);
    Ok(size)
}