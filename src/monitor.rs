// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::types::{Block, Transaction, TransactionStatus};
use crate::MonitorDispatcher;
use anyhow::Result;
use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

use crate::{config::Config, database::Database};

pub struct Monitor {
    config: Config,
    db: Database,
    client: Client,
    dispatcher: Arc<dyn MonitorDispatcher>,
}

impl Monitor {
    pub fn new(dispatcher: Arc<dyn MonitorDispatcher>, config: Config, db: Database) -> Self {
        let client = Client::new();

        Self {
            config,
            db,
            dispatcher,
            client,
        }
    }

    pub async fn run(&self) -> Result<()> {
        info!("Starting Starcoin blockchain monitor...");

        loop {
            match self.monitor_blocks().await {
                Ok(_) => {
                    sleep(Duration::from_millis(
                        self.config.block_subscription_interval,
                    ))
                    .await;
                }
                Err(e) => {
                    error!("Error monitoring blocks: {}", e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn monitor_blocks(&self) -> Result<()> {
        // Get latest block number
        let latest_block = self.get_latest_block_number().await?;
        info!("Latest block number: {}", latest_block);

        // Get block details
        let block_data = self.get_block_by_number(latest_block).await?;

        if let Some(block) = block_data {
            // Process transactions in the block
            for transaction in block.transactions {
                // Check if it's a large transaction
                if transaction.amount >= self.config.min_transaction_amount {
                    info!(
                        "Large transaction detected: {} STC from {} to {}",
                        transaction.amount / 1_000_000_000,
                        transaction.from_address,
                        transaction.to_address
                    );

                    // Save to database
                    self.db.save_transaction(&transaction).await?;

                    // Check if alert already sent
                    if !self.db.is_alert_sent(&transaction.hash).await? {
                        // Send Telegram alert
                        self.send_large_transaction_alert(&transaction).await?;

                        // Mark alert as sent
                        self.db.mark_alert_sent(&transaction.hash).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_latest_block_number(&self) -> Result<u64> {
        let response = self
            .client
            .post(&format!(
                "{}/jsonrpc",
                self.config.starcoin_rpc_url.replace("ws://", "http://")
            ))
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "chain.info",
                "params": []
            }))
            .send()
            .await?;

        let data: Value = response.json().await?;

        if let Some(result) = data.get("result") {
            if let Some(head) = result.get("head") {
                if let Some(number) = head.get("number") {
                    return Ok(number.as_u64().unwrap_or(0));
                }
            }
        }

        Err(anyhow::anyhow!("Failed to get latest block number"))
    }

    async fn get_block_by_number(&self, block_number: u64) -> Result<Option<Block>> {
        let response = self
            .client
            .post(&format!(
                "{}/jsonrpc",
                self.config.starcoin_rpc_url.replace("ws://", "http://")
            ))
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "chain.get_block_by_number",
                "params": [block_number, true]
            }))
            .send()
            .await?;

        let data: Value = response.json().await?;

        if let Some(result) = data.get("result") {
            if let Some(block) = result.as_object() {
                return self.parse_block_data(block).await;
            }
        }

        Ok(None)
    }

    async fn parse_block_data(
        &self,
        block_data: &serde_json::Map<String, Value>,
    ) -> Result<Option<Block>> {
        let block_number = block_data
            .get("header")
            .and_then(|h| h.get("number"))
            .and_then(|n| n.as_u64())
            .ok_or_else(|| anyhow::anyhow!("Invalid block number"))?;

        let block_hash = block_data
            .get("header")
            .and_then(|h| h.get("id"))
            .and_then(|id| id.as_str())
            .unwrap_or("")
            .to_string();

        let timestamp = block_data
            .get("header")
            .and_then(|h| h.get("timestamp"))
            .and_then(|t| t.as_u64())
            .map(|_ts| Utc::now()) // In a real implementation, you'd parse the actual timestamp
            .unwrap_or_else(Utc::now);

        let mut transactions = Vec::new();

        if let Some(txs) = block_data.get("body") {
            if let Some(tx_list) = txs.as_array() {
                for tx_data in tx_list {
                    if let Some(transaction) = self
                        .parse_transaction_data(tx_data, block_number, timestamp)
                        .await?
                    {
                        transactions.push(transaction);
                    }
                }
            }
        }

        Ok(Some(Block {
            number: block_number,
            hash: block_hash,
            timestamp,
            transactions,
        }))
    }

    async fn parse_transaction_data(
        &self,
        tx_data: &Value,
        block_number: u64,
        block_timestamp: chrono::DateTime<Utc>,
    ) -> Result<Option<Transaction>> {
        // This is a simplified parser - in a real implementation, you'd need to parse
        // the actual Starcoin transaction format
        let tx_hash = tx_data
            .get("id")
            .and_then(|id| id.as_str())
            .unwrap_or("")
            .to_string();

        if tx_hash.is_empty() {
            return Ok(None);
        }

        // For demo purposes, we'll create a mock transaction
        // In reality, you'd parse the actual transaction data
        let transaction = Transaction {
            hash: tx_hash,
            block_number,
            timestamp: block_timestamp,
            from_address: "0x0000000000000000000000000000000000000000".to_string(),
            to_address: "0x0000000000000000000000000000000000000000".to_string(),
            amount: 1_000_000_000, // 1 STC in nano units
            token: "STC".to_string(),
            gas_used: 0,
            gas_price: 0,
            status: TransactionStatus::Success,
        };

        Ok(Some(transaction))
    }

    async fn send_large_transaction_alert(&self, transaction: &Transaction) -> Result<()> {
        let amount_stc = transaction.amount as f64 / 1_000_000_000.0;

        let message = format!(
            "🚨 **Large Transaction Alert** 🚨\n\n\
            **Amount:** {:.2} STC\n\
            **From:** `{}`\n\
            **To:** `{}`\n\
            **Block:** {}\n\
            **Hash:** `{}`\n\
            **Time:** {}\n\n\
            💰 This transaction exceeds the minimum threshold of {:.2} STC",
            amount_stc,
            transaction.from_address,
            transaction.to_address,
            transaction.block_number,
            transaction.hash,
            transaction.timestamp.format("%Y-%m-%d %H:%M:%S UTC"),
            self.config.min_transaction_amount as f64 / 1_000_000_000.0
        );

        self.dispatcher.dispatch_msg(message).await?;
        info!("Large transaction alert sent to Telegram");

        Ok(())
    }
}
