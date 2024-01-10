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

mod file_server;

pub use file_server::FileServer;

fn calc_file_size(len: u64) -> String {
    if len < 1024 {
        return format!("{}B", len);
    } else if len < 1024 * 1024 {
        return format!("{}K", len / 1024);
    } else if len < 1024 * 1024 * 1024 {
        return format!("{}M", len / (1024 * 1024));
    } else {
        return format!("{}G", len / (1024 * 1024 * 1024));
    }
}