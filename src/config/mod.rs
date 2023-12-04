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


mod size;
mod duration;
mod log;
mod header;

use std::{str::FromStr, fmt::{Display, self}, marker::PhantomData};

pub use self::size::ConfigSize;
pub use self::duration::ConfigDuration;
pub use self::log::ConfigLog;
pub use self::header::{ConfigHeader, HeaderOper};

use serde::{Serializer, Deserializer, de::{Visitor, Error, self}};
use serde_with::{SerializeAs, DeserializeAs};

pub struct DisplayFromStrOrNumber;

impl<T> SerializeAs<T> for DisplayFromStrOrNumber
where
    T: Display,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(source)
    }
}

impl<'de, T> DeserializeAs<'de, T> for DisplayFromStrOrNumber
where
    T: FromStr,
    T::Err: Display,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Helper<S>(PhantomData<S>);
        impl<'de, S> Visitor<'de> for Helper<S>
        where
            S: FromStr,
            <S as FromStr>::Err: Display,
        {
            type Value = S;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "a string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                value.parse::<Self::Value>().map_err(de::Error::custom)
            }

            /// 将数字转成字符串从而能调用FromStr函数
            fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
                where
                    E: Error, {
                format!("{}", v).parse::<Self::Value>().map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_any(Helper(PhantomData))
    }
}