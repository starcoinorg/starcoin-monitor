// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Native bridge: finalized deposits that nobody claimed. One alert per stuck transfer
//! and one recovery message when it is claimed.

use super::rule::UnixSecs;
use anyhow::Result;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const BRIDGE_URL: &str = "http://bridge-indexer-main-main.starcoin-native-bridge.svc.cluster.local";
const MAX_UNCLAIMED_SECS: u64 = 7200;
/// Stuck deposits already handled by hand: STC->ETH nonce 7, ETH->STC nonces 8 and 9.
const IGNORED: [&str; 3] = ["0:7", "10:8", "10:9"];
const ERROR_REPEAT_SECS: u64 = 1200;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Deposit {
    #[serde(default)]
    pub is_finalized: bool,
    pub timestamp_ms: Option<u64>,
    pub amount: Option<serde_json::Value>,
    pub txn_hash: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Transfer {
    pub source_chain_id: Option<serde_json::Value>,
    pub nonce: Option<serde_json::Value>,
    pub source_chain: Option<String>,
    pub destination_chain: Option<String>,
    pub current_status: Option<String>,
    #[serde(default)]
    pub is_complete: bool,
    pub deposit: Option<Deposit>,
}

impl Transfer {
    /// `<source chain id>:<nonce>`, the key the indexer and the ignore list use.
    pub fn key(&self) -> String {
        let s = |v: &Option<serde_json::Value>| match v {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => "?".into(),
        };
        format!("{}:{}", s(&self.source_chain_id), s(&self.nonce))
    }

    fn line(&self) -> String {
        let d = self.deposit.clone().unwrap_or_default();
        format!(
            "{} {} -> {} status={} amount={} tx={}",
            self.key(),
            self.source_chain.as_deref().unwrap_or("?"),
            self.destination_chain.as_deref().unwrap_or("?"),
            self.current_status.as_deref().unwrap_or("?"),
            d.amount.map(|a| a.to_string()).unwrap_or_default(),
            d.txn_hash.unwrap_or_default()
        )
    }
}

/// Finalized, unclaimed deposits older than MAX_UNCLAIMED_SECS, by key, with their age.
pub fn stuck(transfers: &[Transfer], now: UnixSecs) -> BTreeMap<String, (Transfer, u64)> {
    let mut out = BTreeMap::new();
    for t in transfers {
        let key = t.key();
        let Some(dep) = &t.deposit else { continue };
        if IGNORED.contains(&key.as_str()) || t.is_complete || !dep.is_finalized {
            continue;
        }
        let ts = dep.timestamp_ms.unwrap_or(0) / 1000;
        if ts > 0 && now.saturating_sub(ts) >= MAX_UNCLAIMED_SECS {
            out.insert(key, (t.clone(), now - ts));
        }
    }
    out
}

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    transfers: Vec<Transfer>,
    pagination: Option<Pagination>,
}

#[derive(Deserialize)]
struct Pagination {
    total_pages: Option<u64>,
}

async fn fetch(http: &reqwest::Client) -> Result<Vec<Transfer>> {
    let mut out = vec![];
    for page in 1..=20u64 {
        let url = format!("{BRIDGE_URL}/transfers?finalized_only=true&page={page}&page_size=100");
        let p: Page = http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let total = p.pagination.and_then(|p| p.total_pages).unwrap_or(0);
        let empty = p.transfers.is_empty();
        out.extend(p.transfers);
        if empty || page >= total {
            break;
        }
    }
    Ok(out)
}

fn hours(secs: u64) -> String {
    format!("{}h{}m", secs / 3600, secs % 3600 / 60)
}

#[derive(Default)]
pub struct Bridge {
    stuck: BTreeSet<String>,
    last_error: Option<UnixSecs>,
}

impl Bridge {
    /// Returns the messages to send for this round.
    pub fn observe(&mut self, transfers: &[Transfer], now: UnixSecs) -> Vec<String> {
        self.last_error = None;
        let current = stuck(transfers, now);
        let mut msgs = vec![];
        let new: Vec<_> = current
            .iter()
            .filter(|(k, _)| !self.stuck.contains(*k))
            .collect();
        if !new.is_empty() {
            let mut lines = vec![format!(
                "[ALERT] Starcoin bridge transfers not claimed after {}",
                hours(MAX_UNCLAIMED_SECS)
            )];
            lines.extend(
                new.iter()
                    .map(|(_, (t, age))| format!("{} age={}", t.line(), hours(*age))),
            );
            msgs.push(lines.join("\n"));
        }
        let claimed: Vec<_> = transfers
            .iter()
            .filter(|t| self.stuck.contains(&t.key()) && !current.contains_key(&t.key()))
            .collect();
        if !claimed.is_empty() {
            let mut lines = vec!["[RECOVERY] Starcoin bridge transfers claimed".to_string()];
            lines.extend(claimed.iter().map(|t| t.line()));
            msgs.push(lines.join("\n"));
        }
        self.stuck = current.into_keys().collect();
        msgs
    }

    fn failed(&mut self, err: &anyhow::Error, now: UnixSecs) -> Vec<String> {
        let due = self
            .last_error
            .is_none_or(|at| now.saturating_sub(at) >= ERROR_REPEAT_SECS);
        if !due {
            return vec![];
        }
        self.last_error = Some(now);
        vec![format!(
            "[ALERT] Starcoin bridge monitor cannot read the indexer\n{err}"
        )]
    }

    pub async fn run(&mut self, http: &reqwest::Client, now: UnixSecs) -> Vec<String> {
        match fetch(http).await {
            Ok(transfers) => {
                let msgs = self.observe(&transfers, now);
                tracing::info!(
                    "bridge | transfers={} stuck={:?}",
                    transfers.len(),
                    self.stuck
                );
                msgs
            }
            Err(e) => {
                tracing::error!("bridge | {e}");
                self.failed(&e, now)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 100_000;

    fn t(chain: u64, nonce: u64, age: u64, complete: bool, finalized: bool) -> Transfer {
        Transfer {
            source_chain_id: Some(chain.into()),
            nonce: Some(nonce.into()),
            is_complete: complete,
            deposit: Some(Deposit {
                is_finalized: finalized,
                timestamp_ms: Some((NOW - age) * 1000),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn only_old_finalized_unclaimed_and_not_ignored() {
        let ts = [
            t(0, 1, 8000, false, true),
            t(0, 2, 100, false, true),
            t(0, 3, 8000, true, true),
            t(0, 4, 8000, false, false),
            t(0, 7, 8000, false, true),
            t(10, 8, 9000, false, true),
        ];
        assert_eq!(stuck(&ts, NOW).into_keys().collect::<Vec<_>>(), vec!["0:1"]);
    }

    #[test]
    fn alerts_once_then_recovers() {
        let mut b = Bridge::default();
        let first = b.observe(&[t(0, 1, 8000, false, true)], NOW);
        assert_eq!(first.len(), 1);
        assert!(first[0].starts_with("[ALERT]"));
        assert!(b.observe(&[t(0, 1, 8000, false, true)], NOW).is_empty());
        let done = b.observe(&[t(0, 1, 8000, true, true)], NOW);
        assert_eq!(done.len(), 1);
        assert!(done[0].starts_with("[RECOVERY]"));
    }

    #[test]
    fn string_keys_match_the_ignore_list() {
        let mut x = t(0, 7, 8000, false, true);
        x.source_chain_id = Some("0".into());
        x.nonce = Some("7".into());
        assert_eq!(x.key(), "0:7");
        assert!(stuck(&[x], NOW).is_empty());
    }
}
