use std::{io, fmt::Debug};

use tokio::{net::TcpStream, io::{AsyncRead, AsyncWrite}};
use webparse::{WebError, BinaryMut};
use wenmeng::ProtError;

// #[derive(Debug)]
pub enum ProxyError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    IoError(io::Error),
    WebError(WebError),
    ProtError(ProtError),
    /// 该错误发生协议不可被解析, 则尝试下一个协议
    Continue((Option<BinaryMut>, T)),
    VerifyFail,
    UnknownHost,
    SizeNotMatch,
    TooShort,
    ProtErr,
    ProtNoSupport,
    Extension(&'static str)
}

impl<T> ProxyError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    pub fn extension(value: &'static str) -> ProxyError<T> {
        ProxyError::Extension(value)
    }

    pub fn is_weberror(&self) -> bool {
        match self {
            ProxyError::WebError(_) => true,
            _ => false,
        }
    }
    pub fn to_type<B>(self) -> ProxyError<B> 
    where B : AsyncRead + AsyncWrite + Unpin{
        match self {
            ProxyError::IoError(e) => ProxyError::IoError(e),
            ProxyError::WebError(e) => ProxyError::WebError(e),
            ProxyError::ProtError(e) => ProxyError::ProtError(e),
            ProxyError::Continue(_) => unreachable!("continue can't convert"),
            ProxyError::VerifyFail => ProxyError::VerifyFail,
            ProxyError::UnknownHost => ProxyError::UnknownHost,
            ProxyError::SizeNotMatch => ProxyError::SizeNotMatch,
            ProxyError::TooShort => ProxyError::TooShort,
            ProxyError::ProtErr => ProxyError::ProtErr,
            ProxyError::ProtNoSupport => ProxyError::ProtNoSupport,
            ProxyError::Extension(s) => ProxyError::Extension(s),
        }
    }


}
 
pub type ProxyResult<T> = Result<T, ProxyError<TcpStream>>;
pub type ProxyTypeResult<T, B> = Result<T, ProxyError<B>>;


impl<T> From<io::Error> for ProxyError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    fn from(value: io::Error) -> Self {
        ProxyError::IoError(value)
    }
}

impl<T> From<WebError> for ProxyError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    fn from(value: WebError) -> Self {
        ProxyError::WebError(value)
    }
}

impl<T> From<ProtError> for ProxyError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    fn from(value: ProtError) -> Self {
        ProxyError::ProtError(value)
    }
}

impl<T> Debug for ProxyError<T>
where T : AsyncRead + AsyncWrite + Unpin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(arg0) => f.debug_tuple("IoError").field(arg0).finish(),
            Self::WebError(arg0) => f.debug_tuple("WebError").field(arg0).finish(),
            Self::ProtError(arg0) => f.debug_tuple("ProtErr").field(arg0).finish(),
            Self::Continue(_arg0) => f.debug_tuple("Continue").finish(),
            Self::VerifyFail => write!(f, "VerifyFail"),
            Self::UnknownHost => write!(f, "UnknownHost"),
            Self::SizeNotMatch => write!(f, "SizeNotMatch"),
            Self::TooShort => write!(f, "TooShort"),
            Self::ProtErr => write!(f, "ProtErr"),
            Self::ProtNoSupport => write!(f, "ProtNoSupport"),
            Self::Extension(arg0) => f.debug_tuple("Extension").field(arg0).finish(),
        }
    }
}
