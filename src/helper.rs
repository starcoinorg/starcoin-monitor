// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use starcoin_rpc_api::types::TransactionPayloadView;
use starcoin_rpc_api::{
    chain::GetTransactionOption,
    types::{BlockTransactionsView, BlockView, SignedUserTransactionView},
};
use starcoin_rpc_client::RpcClient;
use std::sync::Arc;
use tracing::info;

pub async fn extract_full_txn_from_block_view(
    rpc_client: Arc<RpcClient>,
    block_views: Vec<BlockView>,
) -> Result<Vec<SignedUserTransactionView>> {
    if block_views.is_empty() {
        return Ok(Vec::new());
    }

    // Collect all transactions that need to be processed
    let mut all_transactions = Vec::new();

    // First, collect all transaction hashes that need to be fetched
    let mut txn_hashes_to_fetch = Vec::new();
    let mut full_transactions = Vec::new();

    info!(
        "do_handle_blocks | hashes to fetch, blocking_views size {}",
        block_views.len()
    );

    for block in &block_views {
        match &block.body {
            BlockTransactionsView::Hashes(txn_hashes) => {
                info!(
                    "extract_full_txn_from_block_view | hashes to fetch, blocking_views size {}",
                    block_views.len()
                );
                txn_hashes_to_fetch.extend(txn_hashes.iter().cloned());
            }
            BlockTransactionsView::Full(txs) => {
                full_transactions.extend(txs.iter().cloned());
            }
        }
    }

    // Now fetch detailed transaction info for hash types
    for txn_hash in txn_hashes_to_fetch {
        let rpc_client_clone = rpc_client.clone();
        let txn_result = tokio::task::spawn_blocking(move || {
            rpc_client_clone
                .chain_get_transaction(txn_hash, Some(GetTransactionOption { decode: true }))
                .ok()
                .flatten()
        })
        .await?;

        if let Some(txn) = txn_result {
            if let Some(user_txn) = txn.user_transaction {
                all_transactions.push(user_txn);
            }
        }
    }

    all_transactions.extend(full_transactions);
    Ok(all_transactions)
}

pub fn parse_txn_p2p_amount(txn_view: SignedUserTransactionView) -> Result<Option<u128>> {
    let txn_payload_view = txn_view
        .raw_txn
        .decoded_payload
        .ok_or(anyhow!("should decode txn"))?;
    let amount = match txn_payload_view {
        TransactionPayloadView::ScriptFunction(function_view) => {
            let module_name = function_view.module.0.to_string();
            let function_name = function_view.function.as_str();

            info!(
                "parse_txn_p2p_amount | script function: {}, {}, args: {:?}",
                function_view.module.0.to_string(),
                function_view.function.as_str(),
                function_view.args
            );
            if module_name == "0x00000000000000000000000000000001::TransferScripts"
                && function_name == "peer_to_peer_v2"
            {
                function_view.args[1].0.as_u64().map(|n| n as u128)
            } else if module_name == "0x00000000000000000000000000000001::TransferScripts"
                && function_name == "peer_to_peer"
            {
                function_view.args[2].0.as_u64().map(|n| n as u128)
            } else {
                None
            }
        }
        _ => None,
    };

    Ok(amount)
}
