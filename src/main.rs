// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

mod config;
mod database;
mod monitor;
mod monitor_dispatcher;
mod telegram;
mod types;

use anyhow::Result;
use clap::Parser;
use monitor_dispatcher::*;
use std::sync::Arc;
use tracing::{info, Level};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Log level
    #[arg(short, long, default_value = "info")]
    log_level: Level,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(args.log_level)
        .init();

    info!("Starting Starcoin Monitor Service...");

    // Load configuration
    let config = config::Config::load()?;
    info!("Configuration loaded successfully");

    // Initialize database
    let db = database::Database::new(&config.database_url).await?;
    info!("Database initialized successfully");

    let telegram_bot = Arc::new(telegram::TelegramBot::new(config.clone(), db.clone()));

    // Start the monitoring service
    let monitor = monitor::Monitor::new(telegram_bot.clone(), config.clone(), db.clone());

    // Run both services concurrently
    tokio::try_join!(monitor.run(), telegram_bot.run())?;

    Ok(())
}
