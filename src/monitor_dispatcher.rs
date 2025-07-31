// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use starcoin_rpc_api::types::{BlockView, TransactionEventView};

pub trait MonitorDispatcher: Send + Sync {
    fn dispatch_event(&self, event: &TransactionEventView) -> anyhow::Result<()>;

    fn dispatch_block(&self, block: BlockView) -> anyhow::Result<()>;
}
