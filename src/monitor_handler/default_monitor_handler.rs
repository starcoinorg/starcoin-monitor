// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{config::Config, helper, monitor_dispatcher::MonitorDispatcher, telegram::TelegramBot};
use anyhow::Result;
use starcoin_rpc_api::types::{BlockView, TransactionEventView};
use starcoin_rpc_client::RpcClient;
use starcoin_types::block::BlockNumber;
use std::sync::Arc;
use tracing::info;

pub struct DefaultMonitorHandler {
    config: Arc<Config>,
    tg_bot: Arc<TelegramBot>,
    rpc_client: Arc<RpcClient>,
}

impl DefaultMonitorHandler {
    pub fn new(rpc_client: Arc<RpcClient>, tg_bot: Arc<TelegramBot>, config: Arc<Config>) -> Self {
        Self {
            rpc_client,
            tg_bot,
            config,
        }
    }
}

#[async_trait::async_trait]
impl MonitorDispatcher for DefaultMonitorHandler {
    async fn dispatch_event(&self, _event: &TransactionEventView) -> Result<()> {
        Ok(())
    }

    async fn dispatch_block(&self, block_view: &BlockView) -> Result<()> {
        let height = block_view.header.number.0;
        info!("dispatch_block | New block arrived: {}", height);
        if block_view.body.txn_hashes().is_empty() {
            return Ok(());
        }

        info!(
            "dispatch_block | The block have transactions, count: {}",
            block_view.body.txn_hashes().len()
        );
        let full_txns = helper::extract_full_txn_from_block_view(
            self.rpc_client.clone(),
            vec![block_view.clone()],
        )
        .await?;

        for txn in full_txns {
            let txn_hash = txn.transaction_hash;
            if let Some(amount) = helper::parse_txn_p2p_amount(txn)? {
                if amount > self.config.min_transaction_amount {
                    let msg = format!(
                        "🚨[大交易事件告警]: 区块: https://stcscan.io/main/blocks/height/{:?}, 交易: https://stcscan.io/main/transactions/detail/{:?}, 额度: {:.9}",
                        height,
                        txn_hash.to_hex_literal(),
                        amount as f64 / 1e9
                    );
                    self.tg_bot.send_message(msg.as_str()).await?;
                    // TODO: write into db
                }
            }
        }

        Ok(())
    }

    async fn dispatch_stcscan_index_exception(
        &self,
        curr_number: BlockNumber,
        cached_number: BlockNumber,
    ) -> Result<()> {
        let msg = format!(
            "🚨[索引差异过大事件告警]: 当前链上区块号: {}, StcScan 缓存的区块号: {}, 差额：{} 其差异过大可能导致StcScan索引追不上 ",
            curr_number,
            cached_number,
            curr_number - cached_number
        );
        self.tg_bot.send_message(msg.as_str()).await
    }
}
