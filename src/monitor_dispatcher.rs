// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;

#[async_trait]
pub trait MonitorDispatcher: Send + Sync {
    async fn dispatch_msg(&self, msg: String) -> anyhow::Result<()>;
}
