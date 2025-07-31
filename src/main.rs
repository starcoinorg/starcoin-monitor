// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

mod config;
mod database;
mod monitor;
mod monitor_dispatcher;
mod pubsub_client;
mod telegram;
mod types;

use anyhow::Result;
use clap::Parser;
use monitor_dispatcher::*;
use starcoin_rpc_api::types::{BlockView, TransactionEventView};
use std::sync::Arc;
use tracing::{info, Level};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: Level,
}

struct FakeMonitorDispatcher {}

impl MonitorDispatcher for FakeMonitorDispatcher {
    fn dispatch_event(&self, event: &TransactionEventView) -> Result<()> {
        todo!()
    }

    fn dispatch_block(&self, block: BlockView) -> Result<()> {
        todo!()
    }
}

fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(args.log_level)
        .init();

    info!("Starting Starcoin Monitor Service...");

    // Load configuration
    let config = Arc::new(config::Config::load()?);
    info!("Configuration loaded successfully");

    // do some compute-heavy work or call synchronous code
    let monitor = monitor::Monitor::new(Arc::new(FakeMonitorDispatcher {}), config)
        .expect("Failed to create monitor.");

    monitor.run()?;

    info!("Stopping Starcoin Monitor Service...");
    Ok(())
}
