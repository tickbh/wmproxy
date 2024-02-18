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
// Created Date: 2023/11/10 02:21:22

use std::{time::Duration, str::FromStr, io, fmt::Display};


/// 配置时长,从字符串转成时长
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDuration(pub Duration);


impl ConfigDuration {
    pub fn new(dur: Duration) -> Self {
        Self(dur)
    }
}

impl From<Duration> for ConfigDuration {
    fn from(value: Duration) -> Self {
        ConfigDuration(value)
    }
}

impl From<ConfigDuration> for Duration {
    fn from(value: ConfigDuration) -> Duration {
        value.0
    }
}

impl FromStr for ConfigDuration {
    type Err=io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
        }

        let d = if s.ends_with("ms") {
            let new = s.trim_end_matches("ms");
            let s = new.parse::<u64>().ok().unwrap_or(1u64);
            Duration::new(0, (s * 1000_000) as u32)
        } else if s.ends_with("h") {
            let new = s.trim_end_matches("h");
            let s = new.parse::<u64>().unwrap_or(1u64);
            Duration::new(s * 3600, 0)
        } else if s.ends_with("min") {
            let new = s.trim_end_matches("min");
            let s = new.parse::<u64>().unwrap_or(1u64);
            Duration::new(s * 60, 0)
        } else if s.ends_with("s") {
            let new = s.trim_end_matches("s");
            let s = new.parse::<u64>().unwrap_or(1u64);
            Duration::new(s, 0)
        } else {
            let s = s.parse::<u64>().unwrap_or(1u64);
            Duration::new(s, 0)
        };

        Ok(ConfigDuration(d))
    }
}


impl Display for ConfigDuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ms = self.0.subsec_millis();
        let s = self.0.as_secs();
        if ms > 0 {
            f.write_str(&format!("{}ms", ms as u64 + s * 1000))
        } else {
            if s >= 3600 && s % 3600 == 0 {
                f.write_str(&format!("{}h", s / 3600))
            } else if s >= 60 && s % 60 == 0 {
                f.write_str(&format!("{}min", s / 60))
            } else {
                f.write_str(&format!("{}s", s))
            }
        }
    }
}


#[cfg(test)]
mod tests {
    macro_rules! mdur {
        ($sec:expr, $ms:expr, $buf:expr) => (
            {
                let dur = std::time::Duration::new($sec, $ms * 1000_000);
                let config = crate::ConfigDuration::from(dur);
                assert_eq!(&format!("{}", config), $buf);
                let config1 = $buf.parse::<crate::ConfigDuration>().unwrap();
                assert_eq!(config1, config);
            }
        )
    }

    #[test]
    fn test_display() {
        mdur!(0, 102, "102ms");
        mdur!(1, 102, "1102ms");
        mdur!(1, 0, "1s");
        mdur!(100, 0, "100s");
        mdur!(120, 0, "2min");
        mdur!(170, 0, "170s");
        mdur!(3600, 0, "1h");
        mdur!(7200, 0, "2h");
        mdur!(7500, 0, "125min");
    }

}
