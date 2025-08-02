// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use starcoin_rpc_api::types::{BlockView, TransactionEventView};
use starcoin_types::block::BlockNumber;

#[async_trait::async_trait]
pub trait MonitorDispatcher: Send + Sync {
    async fn dispatch_event(&self, event: &TransactionEventView) -> anyhow::Result<()>;

    async fn dispatch_block(&self, block: &BlockView) -> anyhow::Result<()>;

    async fn dispatch_stcscan_index_exception(
        &self,
        curr_number: BlockNumber,
        cached_number: BlockNumber,
    ) -> anyhow::Result<()>;
}
