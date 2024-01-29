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
// Created Date: 2023/10/16 04:28:22

mod common;
mod http;
mod limit_req;
mod location;
mod matcher;
mod reverse_helper;
mod server;
mod stream;
mod try_paths;
mod upstream;
mod ws;

pub use common::CommonConfig;
pub use http::HttpConfig;
pub use limit_req::{LimitReq, LimitReqMiddleware};
pub use location::LocationConfig;
pub use matcher::Matcher;
pub use reverse_helper::ReverseHelper;
pub use server::ServerConfig;
pub use stream::{StreamConfig, StreamUdp};
pub use try_paths::TryPathsConfig;
pub use upstream::UpstreamConfig;

use std::{
    fmt::{self},
    marker::PhantomData,
    str::FromStr,
};
use webparse::WebError;

use serde::{
    de::{self, MapAccess, Visitor},
    Deserialize, Deserializer,
};

pub fn string_or_struct<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + FromStr<Err = WebError>,
    D: Deserializer<'de>,
{
    struct StringOrStruct<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for StringOrStruct<T>
    where
        T: Deserialize<'de> + FromStr<Err = WebError>,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_str<E>(self, value: &str) -> Result<T, E>
        where
            E: de::Error,
        {
            Ok(FromStr::from_str(value).unwrap())
        }

        fn visit_map<M>(self, map: M) -> Result<T, M::Error>
        where
            M: MapAccess<'de>,
        {
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_any(StringOrStruct(PhantomData))
}
