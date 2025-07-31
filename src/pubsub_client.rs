// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::{ensure, Result};
use futures::{TryStream, TryStreamExt};
use starcoin_rpc_api::types::pubsub::EventFilter;
use starcoin_rpc_client::RpcClient;

use tokio::io::AsyncBufReadExt;
use tracing::info;

pub struct PubSubClient {
    client: RpcClient,
}

fn blocking_display_notification<T, F>(
    mut event_stream: impl TryStream<Ok = T, Error = anyhow::Error> + Unpin,
    display: F,
) where
    F: Fn(&T) -> String,
{
    let rt = tokio::runtime::Builder::new_multi_thread()
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
    pub fn new(rpc_url: &str) -> Result<Self> {
        ensure!(rpc_url.starts_with("ws://") || rpc_url.starts_with("wss://"));
        Ok(Self {
            client: RpcClient::connect_websocket(rpc_url)?,
        })
    }

    pub fn subscribe_new_blocks(&self) -> Result<()> {
        info!("subscribe_new_blocks | Entered");

        let subscription = self.client.subscribe_new_blocks()?;

        blocking_display_notification(subscription, |evt| {
            serde_json::to_string(&evt).expect("should never fail")
        });

        info!("subscribe_new_blocks | Exited");

        Ok(())
    }

    pub fn subscribe_new_events(&self) -> Result<()> {
        info!("subscribe_new_events | Entered");

        let event_filter = EventFilter {
            from_block: None,
            to_block: None,
            event_keys: None,
            addrs: None,
            type_tags: None,
            limit: None,
        };
        let subscription = self.client.subscribe_events(event_filter, true)?;

        blocking_display_notification(subscription, |evt| {
            serde_json::to_string(&evt).expect("should never fail")
        });

        info!("subscribe_new_events | Exited");
        Ok(())
    }
}
