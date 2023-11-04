use std::{fmt::Display, str::FromStr, io, collections::HashMap};
use lazy_static::lazy_static;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSize(pub u64);

const SIZE_UNIT: [&'static str; 6] = ["", "k", "m", "g", "t", "p"];

lazy_static! {
    static ref SIZE_MAP_UNIT: HashMap<u8, u64> = {
        let mut map = HashMap::new();
        map.insert(b'k', 1024);
        map.insert(b'K', 1024);
        map.insert(b'm', 1024u64.pow(2));
        map.insert(b'M', 1024u64.pow(2));
        map.insert(b'g', 1024u64.pow(3));
        map.insert(b'G', 1024u64.pow(3));
        map.insert(b't', 1024u64.pow(4));
        map.insert(b'T', 1024u64.pow(4));
        map.insert(b'p', 1024u64.pow(5));
        map.insert(b'P', 1024u64.pow(5));
        map
    };

    static ref SIZE_TRIM_STR: [char; 10] = ['k', 'K', 'm', 'M', 'g', 'G', 't', 'T', 'p', 'P'];
}

impl ConfigSize {
    pub fn new(size: u64) -> Self {
        Self(size)
    }
}

impl From<u64> for ConfigSize {
    fn from(value: u64) -> Self {
        ConfigSize(value)
    }
}

impl From<ConfigSize> for u64 {
    fn from(value: ConfigSize) -> u64 {
        value.0
    }
}

impl FromStr for ConfigSize {
    type Err=io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() == 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, ""));
        }
        
        if let Some(multi) = SIZE_MAP_UNIT.get(s.as_bytes().last().unwrap()) {
            let new = s.trim_end_matches(&SIZE_TRIM_STR[..]);
            new.parse::<u64>().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "")).and_then(|s| Ok(ConfigSize(s * multi)))
        } else {
            s.parse::<u64>().map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "")).and_then(|s| Ok(ConfigSize(s)))
        }
    }
}


impl Display for ConfigSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut div = self.0;
        let mut idx = 0;
        while div > 1024 && idx < SIZE_UNIT.len() - 1 {
            let temp = div % 1024;
            if temp != 0 {
                break;
            }
            div = div / 1024;
            idx += 1;
        }
        f.write_str(&format!("{}{}", div, SIZE_UNIT[idx]))
    }
}


mod tests {
    macro_rules! msize {
        ($buf:expr, $equal:expr) => (
            {
                let config = crate::ConfigSize::from(($buf) as u64);
                assert_eq!(&format!("{}", config), $equal);

                let config1 = $equal.parse::<crate::ConfigSize>().unwrap();
                assert_eq!(config1, config);
            }
        )
    }

    #[test]
    fn test_display() {
        msize!(102u64, "102");
        msize!(10240u64, "10k");
        msize!(10240u64 * 1024, "10m");
        msize!(10240u64 * 1024 * 1024, "10g");
        msize!(10240u64 * 1024 * 1024 * 1024, "10t");
        msize!(10240u64 * 1024 * 1024 * 1024 * 1024, "10p");
        msize!(10240u64 * 1024 * 1024 * 1024 * 1024 * 1024, "10240p");
    }

}
