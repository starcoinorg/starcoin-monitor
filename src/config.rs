// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub starcoin_rpc_url: String,
    pub telegram_bot_token: String,
    pub telegram_chat_id: String,
    pub database_url: String,
    pub min_transaction_amount: u64,
    pub block_subscription_interval: u64,
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenv::dotenv().ok();

        let config = Config {
            starcoin_rpc_url: env::var("STARCOIN_RPC_URL")
                .unwrap_or_else(|_| "ws://main.seed.starcoin.org:9870".to_string()),
            telegram_bot_token: env::var("TELEGRAM_BOT_TOKEN")
                .expect("TELEGRAM_BOT_TOKEN must be set"),
            telegram_chat_id: env::var("TELEGRAM_CHAT_ID").expect("TELEGRAM_CHAT_ID must be set"),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:starcoin_monitor.db".to_string()),
            min_transaction_amount: env::var("MIN_TRANSACTION_AMOUNT")
                .unwrap_or_else(|_| "1000000000".to_string())
                .parse()
                .unwrap_or(1_000_000_000), // 1 STC in nano units
            block_subscription_interval: env::var("BLOCK_SUBSCRIPTION_INTERVAL")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .unwrap_or(1000),
        };

        Ok(config)
    }
}
