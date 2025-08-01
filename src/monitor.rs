// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{config::Config, monitor_dispatcher::MonitorDispatcher, pubsub_client::PubSubClient};
use anyhow::{ensure, Result};
use std::sync::Arc;
use std::thread::JoinHandle;
use tracing::info;

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

    pub fn run(&self) -> Result<Vec<JoinHandle<()>>> {
        info!("Monitor::run | entered");

        let pubsub_client1 = self.pubsub_client.clone();
        let dispatcher1 = self.dispatcher.clone();
        let event_watch_handle = std::thread::spawn(move || {
            pubsub_client1
                .subscribe_new_events(|evt| {
                    dispatcher1
                        .dispatch_event(evt)
                        .expect("dispatcher should work");
                })
                .expect("should subscribe new events");
        });

        let pubsub_client2 = self.pubsub_client.clone();
        let dispatcher2 = self.dispatcher.clone();
        let block_watch_handle = std::thread::spawn(move || {
            pubsub_client2
                .subscribe_new_blocks(|block_view| dispatcher2.dispatch_block(block_view).unwrap())
                .expect("should subscribe new events");
        });

        let mut handles = Vec::new();
        handles.push(event_watch_handle);
        handles.push(block_watch_handle);

        info!("Monitor::run | Exited");
        Ok(handles)
    }
}
