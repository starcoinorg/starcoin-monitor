// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use futures::{TryStream, TryStreamExt};
use starcoin_rpc_api::types::{pubsub::EventFilter, BlockView, TransactionEventView};
use starcoin_rpc_client::RpcClient;
use std::sync::Arc;

use tokio::io::AsyncBufReadExt;
use tracing::info;

pub struct PubSubClient {
    rpc_client: Arc<RpcClient>,
}

fn blocking_display_notification<T, F>(
    mut event_stream: impl TryStream<Ok = T, Error = anyhow::Error> + Unpin,
    display: F,
) where
    F: Fn(&T) -> String,
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("should able to create tokio runtime");
    let stdin = tokio::io::stdin();
    let mut lines = tokio::io::BufReader::new(stdin).lines();
    rt.block_on(async move {
        loop {
            tokio::select! {
               maybe_quit = lines.next_line()  => {
                   if let Ok(Some(q)) = maybe_quit {
                       if q.as_str() == "q" {
                           break;
                       }
                   }
               }
               try_event = event_stream.try_next() => {
                   match try_event {
                        Ok(None) => break,
                        Ok(Some(evt)) => {
                            println!("{}", display(&evt));
                        }
                        Err(e) => {
                            eprintln!("subscription return err: {}", &e);
                        }
                   }
               }
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

        blocking_display_notification(subscription, |bv| {
            fun(bv);
            serde_json::to_string(&bv).expect("should never fail")
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

        blocking_display_notification(subscription, |evt| {
            fun(evt);
            serde_json::to_string(&evt).expect("should never fail")
        });

        info!("subscribe_new_events | Exited");
        Ok(())
    }
}
