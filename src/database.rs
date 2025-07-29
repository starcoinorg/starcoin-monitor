// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::types::{AccountBalance, Transaction, TransactionSummary};
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use std::sync::Arc;

#[derive(Clone)]
pub struct Database {
    pool: Arc<SqlitePool>,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;

        // Create tables if they don't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS transactions (
                hash TEXT PRIMARY KEY,
                block_number INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                from_address TEXT NOT NULL,
                to_address TEXT NOT NULL,
                amount INTEGER NOT NULL,
                token TEXT NOT NULL,
                gas_used INTEGER NOT NULL,
                gas_price INTEGER NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS account_balances (
                address TEXT NOT NULL,
                balance INTEGER NOT NULL,
                token TEXT NOT NULL,
                last_updated TEXT NOT NULL,
                PRIMARY KEY (address, token)
            )
            "#,
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS large_transaction_alerts (
                transaction_hash TEXT PRIMARY KEY,
                alert_sent BOOLEAN NOT NULL DEFAULT FALSE,
                sent_at TEXT,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self {
            pool: Arc::new(pool),
        })
    }

    pub async fn save_transaction(&self, transaction: &Transaction) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO transactions 
            (hash, block_number, timestamp, from_address, to_address, amount, token, gas_used, gas_price, status, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&transaction.hash)
        .bind(transaction.block_number as i64)
        .bind(transaction.timestamp.to_rfc3339())
        .bind(&transaction.from_address)
        .bind(&transaction.to_address)
        .bind(transaction.amount as i64)
        .bind(&transaction.token)
        .bind(transaction.gas_used as i64)
        .bind(transaction.gas_price as i64)
        .bind(format!("{:?}", transaction.status))
        .bind(Utc::now().to_rfc3339())
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_transactions_by_block_range(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<Vec<Transaction>> {
        let rows = sqlx::query(
            r#"
            SELECT hash, block_number, timestamp, from_address, to_address, amount, token, gas_used, gas_price, status
            FROM transactions 
            WHERE block_number BETWEEN ? AND ?
            ORDER BY block_number DESC, timestamp DESC
            "#,
        )
        .bind(start_block as i64)
        .bind(end_block as i64)
        .fetch_all(&*self.pool)
        .await?;

        let mut transactions = Vec::new();
        for row in rows {
            let timestamp: String = row.get("timestamp");
            let timestamp = DateTime::parse_from_rfc3339(&timestamp)?.with_timezone(&Utc);

            transactions.push(Transaction {
                hash: row.get("hash"),
                block_number: row.get::<i64, _>("block_number") as u64,
                timestamp,
                from_address: row.get("from_address"),
                to_address: row.get("to_address"),
                amount: row.get::<i64, _>("amount") as u64,
                token: row.get("token"),
                gas_used: row.get::<i64, _>("gas_used") as u64,
                gas_price: row.get::<i64, _>("gas_price") as u64,
                status: serde_json::from_str::<crate::types::TransactionStatus>(
                    &row.get::<String, _>("status"),
                )?,
            });
        }

        Ok(transactions)
    }

    pub async fn get_transaction_summary(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> Result<TransactionSummary> {
        let row = sqlx::query(
            r#"
            SELECT 
                COUNT(*) as total_transactions,
                SUM(amount) as total_amount
            FROM transactions 
            WHERE block_number BETWEEN ? AND ?
            "#,
        )
        .bind(start_block as i64)
        .bind(end_block as i64)
        .fetch_one(&*self.pool)
        .await?;

        Ok(TransactionSummary {
            total_transactions: row.get::<i64, _>("total_transactions") as u64,
            total_amount: row.get::<Option<i64>, _>("total_amount").unwrap_or(0) as u64,
            start_block,
            end_block,
            period: format!("Block {} to {}", start_block, end_block),
        })
    }

    pub async fn save_account_balance(&self, balance: &AccountBalance) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO account_balances 
            (address, balance, token, last_updated)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(&balance.address)
        .bind(balance.balance as i64)
        .bind(&balance.token)
        .bind(balance.last_updated.to_rfc3339())
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_account_balance(
        &self,
        address: &str,
        token: &str,
    ) -> Result<Option<AccountBalance>> {
        let row = sqlx::query(
            r#"
            SELECT address, balance, token, last_updated
            FROM account_balances 
            WHERE address = ? AND token = ?
            "#,
        )
        .bind(address)
        .bind(token)
        .fetch_optional(&*self.pool)
        .await?;

        if let Some(row) = row {
            let last_updated: String = row.get("last_updated");
            let last_updated = DateTime::parse_from_rfc3339(&last_updated)?.with_timezone(&Utc);

            Ok(Some(AccountBalance {
                address: row.get("address"),
                balance: row.get::<i64, _>("balance") as u64,
                token: row.get("token"),
                last_updated,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn mark_alert_sent(&self, transaction_hash: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO large_transaction_alerts 
            (transaction_hash, alert_sent, sent_at, created_at)
            VALUES (?, TRUE, ?, ?)
            "#,
        )
        .bind(transaction_hash)
        .bind(Utc::now().to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    pub async fn is_alert_sent(&self, transaction_hash: &str) -> Result<bool> {
        let row = sqlx::query(
            r#"
            SELECT alert_sent FROM large_transaction_alerts 
            WHERE transaction_hash = ?
            "#,
        )
        .bind(transaction_hash)
        .fetch_optional(&*self.pool)
        .await?;

        Ok(row.map(|r| r.get::<bool, _>("alert_sent")).unwrap_or(false))
    }
}
