// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

mod config;
mod database;
mod monitor;
mod monitor_dispatcher;
mod pubsub_client;
mod telegram;
mod types;

use crate::telegram::TelegramBot;
use anyhow::Result;
use clap::Parser;
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

    let tg_bot = Arc::new(TelegramBot::new(config.clone()));

    // do some compute-heavy work or call synchronous code
    let monitor = monitor::Monitor::new(tg_bot.clone(), config).expect("Failed to create monitor.");

    let mut handles = monitor.run()?;
    handles.push(tg_bot.run()?);

    // Join handles
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    info!("Stopping Starcoin Monitor Service...");
    Ok(())
}
