// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

mod config;
mod daily_notification;
mod helper;
mod monitor;
mod monitor_dispatcher;
mod monitor_handler;
mod pubsub_client;
mod stcscan_monitor;
mod stcscan_monitor_index;
mod telegram;
mod types;

use crate::daily_notification::DailyNotificationService;
use crate::monitor_handler::default_monitor_handler::DefaultMonitorHandler;
use crate::telegram::TelegramBot;
use anyhow::{ensure, Result};
use clap::Parser;
use starcoin_rpc_client::RpcClient;
use stcscan_monitor::StcScanMonitor;
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
    let tg_bot = Arc::new(TelegramBot::new(config.clone(), rpc_client.clone()));
    let monitor_handler = Arc::new(DefaultMonitorHandler::new(
        rpc_client.clone(),
        tg_bot.clone(),
        config.clone(),
    ));

    // Init monitor, do some compute-heavy work or call synchronous code
    let monitor = monitor::Monitor::new(rpc_client.clone(), monitor_handler.clone())
        .expect("Failed to create monitor.");
    let mut handles = monitor.run()?;
    handles.push(tg_bot.run()?);

    // Init stc scan monitor
    let stc_scan_monitor =
        StcScanMonitor::new(config.clone(), monitor_handler.clone(), rpc_client.clone());
    handles.push(stc_scan_monitor.run()?);

    let daily_notification = DailyNotificationService::new(config.clone(), tg_bot.clone());
    handles.push(daily_notification.run()?);
    // Init daily notification service
    // let daily_notification_service = tokio::runtime::Runtime::new()?
    //     .block_on(async { DailyNotificationService::new(config.clone(), tg_bot.clone()).await })?;
    //
    // // Run the async runtime
    // tokio::runtime::Runtime::new()?.block_on(async {
    //     daily_notification_service.start().await?;
    //     Ok::<(), anyhow::Error>(())
    // })?;

    // Join handles
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    info!("Stopping Starcoin Monitor Service...");
    Ok(())
}
