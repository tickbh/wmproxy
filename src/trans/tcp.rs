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
// Created Date: 2023/10/07 09:40:42

use std::sync::Arc;

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite},
    sync::{mpsc::{Sender, channel}, RwLock},
};

use crate::{ProtFrame, TransStream, ProxyError, ProtCreate, MappingConfig};

pub struct TransTcp {
    sender: Sender<ProtFrame>,
    sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
    sock_map: u32,
    mappings: Arc<RwLock<Vec<MappingConfig>>>,
}

impl TransTcp {
    pub fn new(
        sender: Sender<ProtFrame>,
        sender_work: Sender<(ProtCreate, Sender<ProtFrame>)>,
        sock_map: u32,
        mappings: Arc<RwLock<Vec<MappingConfig>>>,
    ) -> Self {
        Self {
            sender,
            sender_work,
            sock_map,
            mappings,
        }
    }

    pub async fn process<T>(self, inbound: T) -> Result<(), ProxyError<T>>
    where
        T: AsyncRead + AsyncWrite + Unpin,
    {
        // 寻找是否有匹配的tcp转发协议，如果有，则进行转发，如果没有则丢弃数据
        {
            let mut is_find = false;
            let read = self.mappings.read().await;

            for v in &*read {
                if v.mode == "tcp" {
                    is_find = true;
                }
            }
            if !is_find {
                log::warn!("未找到正确的tcp商户端映射");
                return Ok(());
            }
        }

        // 通知客户端数据进行连接的建立，客户端的tcp配置只能存在有且只有一个，要不然无法确定转发源
        let create = ProtCreate::new(self.sock_map, None);
        let (stream_sender, stream_receiver) = channel::<ProtFrame>(10);
        let _ = self.sender_work.send((create, stream_sender)).await;
        
        let trans = TransStream::new(inbound, self.sock_map, self.sender, stream_receiver);
        trans.copy_wait().await?;
        Ok(())
    }
}
