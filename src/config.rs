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
    pub telegram_proxy: Option<String>,
    pub min_transaction_amount: u128,
    pub block_subscription_interval: u64,
    pub es_url: String,
    pub es_user_name: String,
    pub es_password: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        dotenv::dotenv().ok();

        let config = Config {
            starcoin_rpc_url: env::var("STARCOIN_RPC_URL")
                .unwrap_or_else(|_| "ws://main.seed.starcoin.org:9870".to_string()),
            telegram_bot_token: env::var("TELEGRAM_BOT_TOKEN")
                .expect("TELEGRAM_BOT_TOKEN is not set"),
            telegram_chat_id: env::var("TELEGRAM_CHAT_ID").expect("TELEGRAM_CHAT_ID is not set"),
            telegram_proxy: env::var("TELOXIDE_PROXY").map(Some).or_else(|e| match e {
                env::VarError::NotPresent => Ok(None),
                _ => Err(e),
            })?,
            min_transaction_amount: env::var("MIN_TRANSACTION_AMOUNT")
                .unwrap_or_else(|_| "1000000000".to_string())
                .parse()
                .unwrap_or(100_000_000), // 1 STC in nano units
            block_subscription_interval: env::var("BLOCK_SUBSCRIPTION_INTERVAL")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .unwrap_or(1000),
            es_url: env::var("ES_URL").unwrap_or_else(|_| "elastic".to_string()),
            es_user_name: env::var("ES_USER_NAME").unwrap_or_else(|_| "elastic".to_string()),
            es_password: env::var("ES_PASSWORD").unwrap_or_else(|_| "changeme".to_string()),
        };

        Ok(config)
    }
}
