// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

mod config;
mod index_monitor_logic;
mod monitor;
mod monitor_dispatcher;
mod pubsub_client;
mod stc_scan_monitor;
mod telegram;
mod types;

use crate::telegram::TelegramBot;
use anyhow::{ensure, Result};
use clap::Parser;
use starcoin_rpc_client::RpcClient;
use stc_scan_monitor::StcScanMonitor;
use std::sync::Arc;
use tracing::{info, Level};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: Level,
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

    let rpc_url = &config.starcoin_rpc_url;
    ensure!(rpc_url.starts_with("ws://") || rpc_url.starts_with("wss://"));
    let rpc_client = Arc::new(RpcClient::connect_websocket(rpc_url)?);

    // Init telegram bot
    let tg_bot = Arc::new(TelegramBot::new(config.clone()));

    // Init monitor, do some compute-heavy work or call synchronous code
    let monitor = monitor::Monitor::new(rpc_client.clone(), tg_bot.clone(), config.clone())
        .expect("Failed to create monitor.");
    let mut handles = monitor.run()?;
    handles.push(tg_bot.run()?);

    // Init stc scan monitor
    let stc_scan_monitor = StcScanMonitor::new(config.clone(), tg_bot.clone(), rpc_client.clone());
    handles.push(stc_scan_monitor.run()?);

    // Join handles
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    info!("Stopping Starcoin Monitor Service...");
    Ok(())
}
