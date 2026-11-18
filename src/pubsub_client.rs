// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use futures::{TryStream, TryStreamExt};
use starcoin_rpc_api::types::{pubsub::EventFilter, BlockView, TransactionEventView};
use starcoin_rpc_client::RpcClient;
use std::sync::Arc;

use tracing::info;

pub struct PubSubClient {
    rpc_client: Arc<RpcClient>,
}

fn blocking_handle_notification<T, F>(
    mut event_stream: impl TryStream<Ok = T, Error = anyhow::Error> + Unpin,
    handle: F,
) where
    F: Fn(&T),
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("should able to create tokio runtime");
    rt.block_on(async move {
        loop {
            match event_stream.try_next().await {
                Ok(None) => break,
                Ok(Some(evt)) => handle(&evt),
                Err(e) => eprintln!("subscription return err: {}", &e),
            }
        }
    });
}
impl PubSubClient {
    pub fn new(rpc_client: Arc<RpcClient>) -> Result<Self> {
        Ok(Self { rpc_client })
    }

    pub fn subscribe_new_blocks<F: Fn(&BlockView)>(&self, fun: F) -> Result<()> {
        info!("subscribe_new_blocks | Entered");

        let subscription = self.rpc_client.subscribe_new_blocks()?;

        blocking_handle_notification(subscription, |bv| {
            fun(bv);
        });

        info!("subscribe_new_blocks | Exited");

        Ok(())
    }

    pub fn subscribe_new_events<F: Fn(&TransactionEventView)>(&self, fun: F) -> Result<()> {
        info!("subscribe_new_events | Entered");

        let event_filter = EventFilter {
            from_block: None,
            to_block: None,
            event_keys: None,
            addrs: None,
            type_tags: None,
            limit: None,
        };
        let subscription = self.rpc_client.subscribe_events(event_filter, true)?;

        blocking_handle_notification(subscription, |evt| {
            fun(evt);
        });

        info!("subscribe_new_events | Exited");
        Ok(())
    }
}
