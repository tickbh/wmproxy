
use std::{fmt::Display, str::FromStr, net::TcpStream};

use crate::ProxyError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigLog {
    pub name: String,
    pub format: String,
}

impl ConfigLog {
    pub fn new(name: String, format: String) -> Self {
        Self {
            name,
            format
        }
    }
}

impl Display for ConfigLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{} {}", self.name, self.format))
    }
}

impl FromStr for ConfigLog {
    type Err=ProxyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let v: Vec<&str> = s.splitn(2, ' ').collect();
        if v.len() < 2 {
            return Err(ProxyError::Extension("名称的格式间必须有空格"));
        }
        let name = v[0].to_string();
        let format = v[1].to_string();
        Ok(Self::new(name, format))
    }
}
