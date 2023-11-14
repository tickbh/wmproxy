use std::fmt;

use log::{Record, Level, Metadata};
use webparse::{Request, Response};
use wenmeng::RecvStream;

#[derive(Debug, Clone)]
pub struct ProxyRecord<'a> {
    pub record: Record<'a>,
    pub req: Option<&'a Request<RecvStream>>,
    pub res: Option<&'a Response<RecvStream>>,
}

impl<'a> ProxyRecord<'a> {
    pub fn new(record: Record<'a>) -> Self {
        Self {
            record,
            req: None,
            res: None,
        }
    }

    pub fn new_req(record: Record<'a>, req: &'a Request<RecvStream>) -> Self {
        Self {
            record,
            req: Some(req),
            res: None,
        }
    }
    
    pub fn new_res(record: Record<'a>, res: &'a Response<RecvStream>) -> Self {
        Self {
            record,
            req: None,
            res: Some(res),
        }
    }

    #[inline]
    pub fn args(&self) -> &fmt::Arguments<'a> {
        self.record.args()
    }

    #[inline]
    pub fn metadata(&self) -> &Metadata<'a> {
        self.record.metadata()
    }

    #[inline]
    pub fn level(&self) -> Level {
        self.record.level()
    }

    #[inline]
    pub fn target(&self) -> &'a str {
        self.record.target()
    }

    #[inline]
    pub fn module_path(&self) -> Option<&'a str> {
        self.record.module_path()
    }

    /// The module path of the message, if it is a `'static` string.
    #[inline]
    pub fn module_path_static(&self) -> Option<&'static str> {
        self.record.module_path_static()
    }

    #[inline]
    pub fn file(&self) -> Option<&'a str> {
        self.record.file()
    }

    #[inline]
    pub fn file_static(&self) -> Option<&'static str> {
        self.record.file_static()
    }

    /// The line containing the message.
    #[inline]
    pub fn line(&self) -> Option<u32> {
        self.record.line()
    }
}

impl<'a> From<Record<'a>> for ProxyRecord<'a> {
    fn from(value: Record<'a>) -> Self {
        ProxyRecord::new(value)
    }
}
