// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{monitor_dispatcher::MonitorDispatcher, pubsub_client::PubSubClient};
use anyhow::Result;
use starcoin_rpc_client::RpcClient;
use std::{sync::Arc, thread::JoinHandle};
use tracing::info;

pub struct Monitor {
    pubsub_client: Arc<PubSubClient>,
    dispatcher: Arc<dyn MonitorDispatcher>,
}

impl Monitor {
    pub fn new(rpc_client: Arc<RpcClient>, dispatcher: Arc<dyn MonitorDispatcher>) -> Result<Self> {
        Ok(Self {
            dispatcher,
            pubsub_client: Arc::new(PubSubClient::new(rpc_client)?),
        })
    }

    pub fn run(&self) -> Result<Vec<JoinHandle<()>>> {
        info!("Monitor::run | entered");

        let pubsub_client1 = self.pubsub_client.clone();
        let dispatcher1 = self.dispatcher.clone();
        let event_watch_handle = std::thread::spawn(move || {
            pubsub_client1
                .subscribe_new_events(|evt| {
                    let dispatcher = dispatcher1.clone();
                    let evt_clone = evt.clone();
                    tokio::spawn(async move { dispatcher.dispatch_event(&evt_clone).await });
                })
                .expect("should subscribe new events");
        });

        let pubsub_client2 = self.pubsub_client.clone();
        let dispatcher2 = self.dispatcher.clone();
        let block_watch_handle = std::thread::spawn(move || {
            pubsub_client2
                .subscribe_new_blocks(|evt| {
                    let dispatcher = dispatcher2.clone();
                    let evt_clone = evt.clone();
                    tokio::spawn(async move { dispatcher.dispatch_block(&evt_clone).await });
                })
                .expect("should subscribe new events");
        });

        let mut handles = Vec::new();
        handles.push(event_watch_handle);
        handles.push(block_watch_handle);

        info!("Monitor::run | Exited");
        Ok(handles)
    }
}
