// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::config::Config;
use crate::telegram::TelegramBot;
use anyhow::Result;
use base64::Engine;
use chrono::{TimeZone, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::thread::JoinHandle;
use tracing::info;

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TransferDocument {
    amount: String,
    identifier: String,
    receiver: String,
    sender: String,
    timestamp: i64,
    txn_hash: String,
    #[serde(rename = "type_tag")]
    type_tag: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EsSearchResponse {
    hits: EsHits,
}

#[derive(Debug, Serialize, Deserialize)]
struct EsHits {
    total: EsTotal,
    hits: Vec<EsHit>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EsTotal {
    value: i64,
    relation: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct EsHit {
    #[serde(rename = "_source")]
    source: TransferDocument,
}

pub struct DailyNotificationService {
    config: Arc<Config>,
    telegram_bot: Arc<TelegramBot>,
}

impl DailyNotificationService {
    pub fn new(config: Arc<Config>, telegram_bot: Arc<TelegramBot>) -> Self {
        Self {
            config,
            telegram_bot,
        }
    }

    pub fn run(&self) -> Result<JoinHandle<()>> {
        let config = self.config.clone();
        let telegram_bot = self.telegram_bot.clone();
        Ok(std::thread::spawn(move || {
            info!("Starting Daily Notification Service...");

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .enable_io()
                .build()
                .unwrap();

            rt.block_on(async move {
                loop {
                    // Calculate time until next 23:59
                    let now = Utc::now();
                    let target_time = now.date_naive().and_hms_opt(23, 59, 0).unwrap();

                    // If it's already past 23:59 today, schedule for tomorrow
                    let target_time =
                        if now.time() >= chrono::NaiveTime::from_hms_opt(23, 59, 0).unwrap() {
                            target_time + chrono::Duration::days(1)
                        } else {
                            target_time
                        };

                    let duration_until_target = target_time - now.naive_utc();
                    let sleep_duration =
                        std::time::Duration::from_secs(duration_until_target.num_seconds() as u64);

                    info!(
                        "Next daily notification scheduled for: {} (sleeping for {:?})",
                        target_time.format("%Y-%m-%d %H:%M:%S"),
                        sleep_duration
                    );

                    // Sleep until target time
                    tokio::time::sleep(sleep_duration).await;

                    // Execute daily tasks
                    info!("Executing daily notification tasks...");

                    // Query daily transfers
                    match query_daily_transfers(
                        &config.es_url,
                        &config.es_user_name,
                        &config.es_password,
                        config.min_transaction_amount,
                    )
                    .await
                    {
                        Ok(transfers) => {
                            info!("Retrieved {} transfer documents", transfers.len());

                            // Send daily summary
                            if let Err(e) = send_daily_summary(transfers, &telegram_bot).await {
                                tracing::error!("Failed to send daily summary: {}", e);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to query daily transfers: {}", e);
                        }
                    }

                    // Small delay to avoid multiple executions at exactly the same time
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
            });

            info!("Daily Notification Service started successfully");
        }))
    }
}

async fn query_daily_transfers(
    es_url: &str,
    es_user_name: &str,
    es_password: &str,
    mint_trans_amount: u128,
) -> Result<Vec<TransferDocument>> {
    info!("Starting daily ES query for transfers...");

    let client = Client::new();
    let url = format!(
        "{}/main.0727.transfer/_search",
        es_url.trim_end_matches('/')
    );

    // Create basic auth header
    let auth_header = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", es_user_name, es_password))
    );

    // Get today's start timestamp (00:00:00)
    let today = Utc::now().date_naive();
    let today_start = Utc.from_utc_datetime(&today.and_hms_opt(0, 0, 0).unwrap());
    let today_start_timestamp = today_start.timestamp_millis();

    let query = serde_json::json!({
        "query": {
            "range": {
                "timestamp": {
                    "gte": today_start_timestamp
                }
            }
        },
        "size": 1000,
        "sort": [{"timestamp": "asc"}]
    });

    info!("Querying ES with timestamp >= {}", today_start_timestamp);

    let response = client
        .post(&url)
        .header("Authorization", auth_header)
        .header("Content-Type", "application/json")
        .json(&query)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await?;
        anyhow::bail!(
            "Elasticsearch request failed with status: {} - Error body: {}",
            status,
            error_body
        );
    }

    let response_text = response.text().await?;
    let es_response: EsSearchResponse = serde_json::from_str(&response_text)?;

    info!(
        "Retrieved {} transfer documents",
        es_response.hits.hits.len()
    );

    // Process and store the results for later notification
    Ok(process_transfers(&es_response.hits.hits, mint_trans_amount))
}

async fn send_daily_summary(
    daily_results: Vec<TransferDocument>,
    telegram_bot: &TelegramBot,
) -> Result<()> {
    info!("Sending daily summary notification...");

    if daily_results.is_empty() {
        let message = "📊 **每日交易汇总**\n\n今日没有发现大额交易";
        telegram_bot.send_message(message).await?;
        return Ok(());
    }

    // Calculate summary statistics
    let total_transfers = daily_results.len();
    let total_amount: u128 = daily_results
        .iter()
        .map(|t| parse_hex_amount(&t.amount).unwrap_or(0))
        .sum();

    // Format the message
    let message = format!(
        "📊 **每日交易汇总**\n\n\
            📅 日期: {}\n\
            🔢 大额交易总数: {}\n\
            💰 交易总额: {:.9} STC\n",
        Utc::now().format("%Y-%m-%d"),
        total_transfers,
        (total_amount as f64) / 1e9,
    );

    // Send the message
    telegram_bot.send_message(&message).await?;
    info!("Daily summary notification sent successfully");

    Ok(())
}

fn process_transfers(hits: &[EsHit], min_amount: u128) -> Vec<TransferDocument> {
    let mut large_transfers = Vec::new();

    for hit in hits {
        let transfer = &hit.source;

        // Parse the amount from hex string
        if let Ok(amount) = parse_hex_amount(&transfer.amount) {
            if amount >= min_amount {
                large_transfers.push(transfer.clone());
            }
        }
    }

    large_transfers
}

fn parse_hex_amount(hex_amount: &str) -> Result<u128> {
    // Remove "0x" prefix if present
    let hex_str = if hex_amount.starts_with("0x") {
        &hex_amount[2..]
    } else {
        hex_amount
    };

    // Convert hex string to bytes using hex crate
    let bytes = hex::decode(hex_str)?;

    // The amount is stored as a Move u128 in BCS format
    // BCS stores u128 as 16 bytes in little-endian order
    // We need to reverse the bytes to get the correct value

    if bytes.len() > 16 {
        anyhow::bail!("Amount exceeds u128 size: {} bytes", bytes.len());
    }

    // Pad to 16 bytes if necessary (add zeros at the end for little-endian)
    let mut padded_bytes = vec![0u8; 16];
    for (i, &byte) in bytes.iter().enumerate() {
        if i < 16 {
            padded_bytes[i] = byte;
        }
    }

    // Convert from little-endian bytes to u128
    // In little-endian, the least significant byte is at index 0
    let mut amount = 0u128;
    for (i, &byte) in padded_bytes.iter().enumerate() {
        amount = amount.wrapping_add((byte as u128).wrapping_shl((i * 8) as u32));
    }

    Ok(amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_amount() {
        // Test with actual ES data format
        // These are real amounts from ES, stored as Move u128 in BCS format

        // Test a small amount
        let amount = parse_hex_amount("0x64").unwrap();
        assert_eq!(amount, 100);

        // Test with padding zeros (this should be a small amount)
        let amount = parse_hex_amount("0x0a000000000000000000000000000000").unwrap();
        println!("Parsed amount: {}", amount);
        // This should be a reasonable amount, not necessarily 10 STC

        // Test another example from actual ES data
        let amount = parse_hex_amount("0x583e2993a60100000000000000000000").unwrap();
        println!("Parsed amount: {}", amount);
        // This should be a reasonable amount
    }

    #[test]
    fn test_process_transfers() {
        let min_amount = 1000000000; // 1 STC in nano units

        let hits = vec![
            EsHit {
                source: TransferDocument {
                    amount: "0x0a000000000000000000000000000000".to_string(), // 10 (less than 1 STC)
                    identifier: "peer_to_peer".to_string(),
                    receiver: "0x4a50777e0e4f67625400148b04afd572".to_string(),
                    sender: "0xa77e09f66ea8ed586467e36ce89362b9".to_string(),
                    timestamp: 1621314570704,
                    txn_hash: "0x17188cdb0d7155e75abb126ddc2359d5ac31d686f337118d65f1adc6650d4d37"
                        .to_string(),
                    type_tag: "0x00000000000000000000000000000001::STC::STC".to_string(),
                },
            },
            EsHit {
                source: TransferDocument {
                    amount: "0x583e2993a60100000000000000000000".to_string(), // 1814945152600 (much larger than 1 STC)
                    identifier: "peer_to_peer".to_string(),
                    receiver: "0x4a50777e0e4f67625400148b04afd572".to_string(),
                    sender: "0xa77e09f66ea8ed586467e36ce89362b9".to_string(),
                    timestamp: 1621314570704,
                    txn_hash: "0x17188cdb0d7155e75abb126ddc2359d5ac31d686f337118d65f1adc6650d4d38"
                        .to_string(),
                    type_tag: "0x00000000000000000000000000000001::STC::STC".to_string(),
                },
            },
        ];

        let large_transfers = process_transfers(&hits, min_amount);

        // Only the second transfer should be included (1814945152600 > 1000000000)
        assert_eq!(large_transfers.len(), 1);
        assert_eq!(
            large_transfers[0].txn_hash,
            "0x17188cdb0d7155e75abb126ddc2359d5ac31d686f337118d65f1adc6650d4d38"
        );
    }

    #[ignore]
    #[tokio::test]
    async fn test_parse_hex_amount_error() -> Result<()> {
        let txn_docs = query_daily_transfers(
            "http://127.0.0.1:9200",
            "elastic",
            "passwd",
            100u128 * 1e9 as u128,
        )
        .await?;
        println!("{:?}", txn_docs);

        let total_amount: u128 = txn_docs
            .iter()
            .map(|t| parse_hex_amount(&t.amount).unwrap_or(0))
            .sum();

        println!(
            "{:?}, total amount: {:.9}",
            txn_docs,
            total_amount as f64 / 1e9
        );
        Ok(())
    }
}
