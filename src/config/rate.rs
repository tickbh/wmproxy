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
// Created Date: 2023/12/05 11:39:27

use std::{fmt::Display, io, str::FromStr};
use wenmeng::Rate;

use crate::{ConfigSize, ConfigDuration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigRate(pub Rate);

impl ConfigRate {
    pub fn new(dur: Rate) -> Self {
        Self(dur)
    }
}

impl From<Rate> for ConfigRate {
    fn from(value: Rate) -> Self {
        ConfigRate(value)
    }
}

impl From<ConfigRate> for Rate {
    fn from(value: ConfigRate) -> Rate {
        value.0
    }
}

impl FromStr for ConfigRate {
    type Err = io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
        }

        let rate_key = s.split("/").map(|k| k.trim()).collect::<Vec<&str>>();
        if rate_key.len() == 1 {
            return Err(io::Error::new(io::ErrorKind::Other, "未知的LimitReq"));
        }

        let size = ConfigSize::from_str(rate_key[0])?;
        let duration = ConfigDuration::from_str(rate_key[1])?;
        

        Ok(ConfigRate::new(Rate::new(size.0, duration.0)))
    }
}

impl Display for ConfigRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// mod tests {
//     macro_rules! mdur {
//         ($sec:expr, $ms:expr, $buf:expr) => (
//             {
//                 let dur = Rate::new($sec, $ms * 1000_000);
//                 let config = crate::ConfigRate::from(dur);
//                 assert_eq!(&format!("{}", config), $buf);
//                 let config1 = $buf.parse::<crate::ConfigRate>().unwrap();
//                 assert_eq!(config1, config);
//             }
//         )
//     }

//     #[test]
//     fn test_display() {
//         mdur!(0, 102, "102ms");
//         mdur!(1, 102, "1102ms");
//         mdur!(1, 0, "1s");
//         mdur!(100, 0, "100s");
//         mdur!(120, 0, "2min");
//         mdur!(170, 0, "170s");
//         mdur!(3600, 0, "1h");
//         mdur!(7200, 0, "2h");
//         mdur!(7500, 0, "125min");
//     }

// }
