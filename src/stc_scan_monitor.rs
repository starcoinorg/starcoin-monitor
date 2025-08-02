// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{config::Config, monitor_dispatcher::MonitorDispatcher};
use anyhow::Result;
use base64::Engine;
use chrono::Utc;
use reqwest::Client;
use serde_json::Value;
use starcoin_rpc_client::RpcClient;
use starcoin_types::block::BlockNumber;
use std::sync::Arc;
use std::thread::JoinHandle;
use tracing::log::debug;
use tracing::{error, info};

const MAX_NOTIFY_TIME_INTERVAL: u64 = 600;

const MAC_BLOCK_DIFFER: u64 = 1000;

const SLEEP_INTERVAL_MILLI_SECONDS: u64 = 50000;

pub struct StcScanMonitor {
    config: Arc<Config>,
    dispatcher: Arc<dyn MonitorDispatcher>,
    rpc_client: Arc<RpcClient>,
}

/// get the cached index block number from elastic search,
/// use es name and password from config object
///
/// Reference the command by following
/// ```
/// GET /main.0727.blocks/_mapping
/// {
///   "main.0727.blocks" : {
///     "mappings" : {
///       "_meta" : {
///         "tip" : {
///           "block_hash" : "0xe047e1f3aab8ea99cf455cf7a69002bd12928d2d4d6a7069fde1f66dc67a902d",
///           "block_number" : 648200
///         }
///       },
///   ...
/// }
/// ```
async fn get_cached_index_block_numer(
    es_url: &str,
    es_user: &str,
    es_password: &str,
) -> Result<u64> {
    let client = Client::new();

    // Construct the URL for the mapping endpoint
    let url = format!("{}/main.0727.blocks/_mapping", es_url.trim_end_matches('/'));

    // Create basic auth header
    let auth_header = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", es_user, es_password))
    );

    info!("Fetching cached block number from Elasticsearch: {}", url);

    let response = client
        .get(&url)
        .header("Authorization", auth_header)
        .header("Content-Type", "application/json")
        .send()
        .await?;

    if !response.status().is_success() {
        error!(
            "Elasticsearch request failed with status: {}",
            response.status()
        );
        return Ok(0u64); // Return 0 as fallback
    }

    let response_text = response.text().await?;
    let json: Value = serde_json::from_str(&response_text)?;

    // Navigate to the block_number field in the response
    // Based on the comment, the structure is:
    // {
    //   "main.0727.blocks" : {
    //     "mappings" : {
    //       "_meta" : {
    //         "tip" : {
    //           "block_hash" : "...",
    //           "block_number" : 648200
    //         }
    //       }
    //     }
    //   }
    // }

    let block_number = json
        .get("main.0727.blocks")
        .and_then(|v| v.get("mappings"))
        .and_then(|v| v.get("_meta"))
        .and_then(|v| v.get("tip"))
        .and_then(|v| v.get("block_number"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0u64);

    info!("Retrieved cached block number: {}", block_number);
    Ok(block_number)
}

impl StcScanMonitor {
    pub fn new(
        config: Arc<Config>,
        dispatcher: Arc<dyn MonitorDispatcher>,
        rpc_client: Arc<RpcClient>,
    ) -> Self {
        Self {
            config,
            dispatcher,
            rpc_client,
        }
    }

    pub fn run(&self) -> Result<JoinHandle<()>> {
        let rpc_client = self.rpc_client.clone();
        let dispatcher = self.dispatcher.clone();
        let config = self.config.clone();
        Ok(std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .enable_time()
                .build()
                .unwrap();

            let mut latest_notify_time: u64 = 0;
            loop {
                // Get current block number
                let current_block_number = rpc_client
                    .chain_info()
                    .expect("get current block number")
                    .head
                    .number
                    .0;

                let cached_index_number: BlockNumber = rt.block_on(async {
                    get_cached_index_block_numer(
                        &config.es_url,
                        &config.es_user_name,
                        &config.es_password,
                    )
                    .await
                    .unwrap_or(0)
                });

                debug!(
                    "StcScanMonitor::run | current_block_number: {}, cached_index_number: {}",
                    current_block_number, cached_index_number
                );

                if current_block_number < cached_index_number {
                    rt.block_on(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(10000)).await;
                    });
                    continue;
                }

                let utc_timestamp_secs = Utc::now().timestamp() as u64;

                if (current_block_number - cached_index_number) > MAC_BLOCK_DIFFER
                    && (latest_notify_time != 0
                        || utc_timestamp_secs - latest_notify_time > MAX_NOTIFY_TIME_INTERVAL)
                {
                    let dispatcher_clone = dispatcher.clone();
                    rt.block_on(async move {
                        let _ = dispatcher_clone
                            .dispatch_stcscan_index_exception(
                                current_block_number,
                                cached_index_number,
                            )
                            .await;
                    });
                    latest_notify_time = utc_timestamp_secs;
                }

                std::thread::sleep(std::time::Duration::from_millis(5000));
            }
        }))
    }
}

#[cfg(test)]
mod test {
    use crate::stc_scan_monitor::get_cached_index_block_numer;
    use anyhow::Result;

    #[tokio::test]
    async fn test_get_cached_index_block_numer() -> Result<()> {
        let block_number =
            get_cached_index_block_numer("http://127.0.0.1:9200", "elastic-user", "password")
                .await?;
        println!("Block number: {}", block_number);
        assert_ne!(block_number, 0, "should not equal 0");
        Ok(())
    }
}
