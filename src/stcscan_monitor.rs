// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    config::Config,
    monitor_dispatcher::MonitorDispatcher,
    stcscan_monitor_index::{
        check_index_monitor_state, update_notification_state, IndexMonitorConfig,
        IndexMonitorResult, NotificationState,
    },
};
use anyhow::Result;
use base64::Engine;

use reqwest::Client;
use serde_json::Value;
use starcoin_rpc_client::RpcClient;
use starcoin_types::block::BlockNumber;
use std::{sync::Arc, thread::JoinHandle};
use tracing::{debug, error, info};

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
        let status = response.status();
        let error_body = response
            .text()
            .await
            .unwrap_or_else(|_| "Unable to read error body".to_string());
        anyhow::bail!(
            "Elasticsearch request failed with status: {} - Error body: {}",
            status,
            error_body
        );
    }

    let response_text = response.text().await?;
    info!("Fetching response text: {}", response_text);
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

            let mut notification_state = NotificationState::default();
            let monitor_config = IndexMonitorConfig::default();

            loop {
                // Get current block number
                let current_block_number = rpc_client
                    .chain_info()
                    .expect("get current block number")
                    .head
                    .number
                    .0;

                let cached_index_number: BlockNumber = match rt.block_on(async {
                    get_cached_index_block_numer(
                        &config.es_url,
                        &config.es_user_name,
                        &config.es_password,
                    )
                    .await
                }) {
                    Ok(block_number) => block_number,
                    Err(e) => {
                        error!("Failed to get cached index block number from ES: {}", e);
                        std::thread::sleep(std::time::Duration::from_millis(50000));
                        continue;
                    }
                };

                debug!(
                    "StcScanMonitor::run | current_block_number: {}, cached_index_number: {}",
                    current_block_number, cached_index_number
                );

                // Use the new logic module to determine what action to take
                let monitor_result = check_index_monitor_state(
                    current_block_number,
                    cached_index_number,
                    &notification_state,
                    &monitor_config,
                );

                match monitor_result {
                    IndexMonitorResult::ShouldWait => {
                        rt.block_on(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(50000)).await;
                        });
                        continue;
                    }
                    IndexMonitorResult::ShouldNotify {
                        current_block,
                        cached_block,
                        ..
                    } => {
                        let dispatcher_clone = dispatcher.clone();
                        rt.block_on(async move {
                            let _ = dispatcher_clone
                                .dispatch_stcscan_index_exception(current_block, cached_block)
                                .await;
                        });
                        update_notification_state(&mut notification_state);
                    }
                    IndexMonitorResult::NoAction => {
                        // No action needed, continue to next iteration
                    }
                }

                std::thread::sleep(std::time::Duration::from_millis(50000));
            }
        }))
    }
}

#[cfg(test)]
mod test {
    use crate::stcscan_monitor::get_cached_index_block_numer;
    use anyhow::Result;

    #[ignore]
    #[tokio::test]
    async fn test_get_cached_index_block_numer() -> Result<()> {
        let block_number =
            get_cached_index_block_numer(&"http://127.0.0.1:9200", &"elastic", &"pass").await?;
        assert_ne!(block_number, 0);
        Ok(())
    }

    #[test]
    fn test_index_monitor_logic_integration() {
        use crate::stcscan_monitor_index::{
            check_index_monitor_state, IndexMonitorConfig, NotificationState,
        };

        let config = IndexMonitorConfig::default();
        let mut state = NotificationState::default();

        // Test scenario 1: Current block behind cached block
        let result = check_index_monitor_state(100, 200, &state, &config);
        assert!(matches!(
            result,
            crate::stcscan_monitor_index::IndexMonitorResult::ShouldWait
        ));

        // Test scenario 2: Large difference, should notify
        let result = check_index_monitor_state(1200, 100, &state, &config);
        assert!(matches!(
            result,
            crate::stcscan_monitor_index::IndexMonitorResult::ShouldNotify { .. }
        ));

        // Test scenario 3: Small difference, no action
        let result = check_index_monitor_state(1050, 100, &state, &config);
        assert!(matches!(
            result,
            crate::stcscan_monitor_index::IndexMonitorResult::NoAction
        ));

        // Test scenario 4: After notification, should not notify again immediately
        if let crate::stcscan_monitor_index::IndexMonitorResult::ShouldNotify { .. } = result {
            // This should not happen in this test case
        }

        // Update state to simulate recent notification
        state.latest_notify_time = chrono::Utc::now().timestamp() as u64 - 100;

        // Test scenario 5: Large difference but recent notification
        let result = check_index_monitor_state(1200, 100, &state, &config);
        assert!(matches!(
            result,
            crate::stcscan_monitor_index::IndexMonitorResult::NoAction
        ));
    }
}
