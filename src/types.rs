// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub hash: String,
    pub block_number: u64,
    pub timestamp: DateTime<Utc>,
    pub from_address: String,
    pub to_address: String,
    pub amount: u64,
    pub token: String,
    pub gas_used: u64,
    pub gas_price: u64,
    pub status: TransactionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionStatus {
    Success,
    Failed,
    Pending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub number: u64,
    pub hash: String,
    pub timestamp: DateTime<Utc>,
    pub transactions: Vec<Transaction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub address: String,
    pub balance: u64,
    pub token: String,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeTransactionAlert {
    pub transaction: Transaction,
    pub alert_sent: bool,
    pub sent_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramCommand {
    pub command: String,
    pub args: Vec<String>,
    pub chat_id: String,
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSummary {
    pub total_transactions: u64,
    pub total_amount: u64,
    pub start_block: u64,
    pub end_block: u64,
    pub period: String,
}
