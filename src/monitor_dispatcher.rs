// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use starcoin_rpc_api::types::{BlockView, TransactionEventView};

#[async_trait::async_trait]
pub trait MonitorDispatcher: Send + Sync {
    async fn dispatch_event(&self, event: &TransactionEventView) -> anyhow::Result<()>;

    async fn dispatch_block(&self, block: &BlockView) -> anyhow::Result<()>;
}
