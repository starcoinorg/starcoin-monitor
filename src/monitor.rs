// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{types::Transaction, MonitorDispatcher};

use anyhow::{ensure, Result};
use starcoin_rpc_client::RpcClient;
use std::sync::Arc;

use tracing::info;

use crate::{config::Config, pubsub_client::PubSubClient};

pub struct Monitor {
    pubsub_client: Arc<PubSubClient>,
    dispatcher: Arc<dyn MonitorDispatcher>,
}

impl Monitor {
    pub fn new(dispatcher: Arc<dyn MonitorDispatcher>, config: Arc<Config>) -> Result<Self> {
        let rpc_url = config.starcoin_rpc_url.clone();
        Ok(Self {
            dispatcher,
            pubsub_client: Arc::new(PubSubClient::new(rpc_url.as_str())?),
        })
    }

    pub fn run(&self) -> Result<()> {
        info!("Monitor::run | entered");

        let pubsub_client1 = self.pubsub_client.clone();
        let event_watch_handle = std::thread::spawn(move || {
            pubsub_client1
                .subscribe_new_events()
                .expect("should subscribe new events");
        });

        let pubsub_client2 = self.pubsub_client.clone();
        let block_watch_handle = std::thread::spawn(move || {
            pubsub_client2
                .subscribe_new_blocks()
                .expect("should subscribe new events");
        });

        let mut handles = Vec::new();
        handles.push(event_watch_handle);
        handles.push(block_watch_handle);

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        info!("Monitor::run | Exited");
        Ok(())
    }
}
