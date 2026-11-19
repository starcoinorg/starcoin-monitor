// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Main block production and mining pool payout settlement.

use super::rule::{Finding, UnixSecs};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

/// Polled over HTTP on purpose: this check must not depend on the websocket the
/// large-transfer watcher uses.
const MAIN_RPC_URLS: [&str; 2] = [
    "http://starcoin-svc.starcoin-main.svc.cluster.local",
    "http://main.seed.starcoin.org",
];
const MAX_BLOCK_AGE_SECS: u64 = 1200;
const SETTLEMENT_URL: &str =
    "http://starcoin-pool-service-svc.starcoin-main.svc.cluster.local/v1/mining/settlement-health";
const MAX_SETTLEMENT_DELAY_BLOCKS: u64 = 120;
const MAX_SETTLEMENT_BATCH_AGE_SECS: u64 = 600;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Head {
    pub timestamp: UnixSecs,
    pub number: u64,
    pub hash: String,
}

pub async fn fetch_head(http: &reqwest::Client, url: &str) -> Result<Head> {
    let body = json!({"id": 1, "jsonrpc": "2.0", "method": "chain.info", "params": []});
    let v: Value = http
        .post(url)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let head = &v["result"]["head"];
    let num = |k: &str| -> Result<u64> {
        head[k]
            .as_str()
            .and_then(|s| s.parse().ok())
            .or(head[k].as_u64())
            .context(format!("head.{k}"))
    };
    Ok(Head {
        timestamp: num("timestamp")? / 1000,
        number: num("number")?,
        hash: head["block_hash"].as_str().unwrap_or_default().to_string(),
    })
}

pub fn block_finding(heads: &[Head], errors: &[String], now: UnixSecs) -> Finding {
    let Some(head) = heads.iter().max() else {
        return Finding::new(
            "main-block",
            Some(format!("all RPC checks failed: {}", errors.join("; "))),
            "",
        );
    };
    let age = now.saturating_sub(head.timestamp);
    let detail = format!("height={} age={}s hash={}", head.number, age, head.hash);
    let problem = (age > MAX_BLOCK_AGE_SECS)
        .then(|| format!("no Main block for {age}s (> {MAX_BLOCK_AGE_SECS}s)"));
    Finding::new("main-block", problem, detail)
}

pub async fn check_block(http: &reqwest::Client, now: UnixSecs) -> Vec<Finding> {
    let (mut heads, mut errors) = (vec![], vec![]);
    for url in MAIN_RPC_URLS {
        match fetch_head(http, url).await {
            Ok(h) => heads.push(h),
            Err(e) => errors.push(format!("{url}: {e}")),
        }
    }
    vec![block_finding(&heads, &errors, now)]
}

#[derive(Debug, Clone, Default, Deserialize, serde::Serialize)]
pub struct Settlement {
    pub head_height: Option<u64>,
    pub payout_ready_height: Option<u64>,
    pub last_settled_height: Option<u64>,
    pub settlement_delay_blocks: Option<u64>,
    pub last_batch_age_secs: Option<u64>,
    pub pending_submits: Option<u64>,
    pub pending_candidates: Option<u64>,
}

pub fn settlement_problem(s: &Settlement) -> Option<String> {
    let Some(delay) = s.settlement_delay_blocks else {
        return Some("settlement checkpoint missing".into());
    };
    let Some(batch_age) = s.last_batch_age_secs else {
        return Some("settlement batch checkpoint missing".into());
    };
    if delay > MAX_SETTLEMENT_DELAY_BLOCKS {
        return Some(format!(
            "settlement delay {delay} blocks > {MAX_SETTLEMENT_DELAY_BLOCKS}"
        ));
    }
    if batch_age > MAX_SETTLEMENT_BATCH_AGE_SECS {
        return Some(format!(
            "settlement batch age {batch_age}s > {MAX_SETTLEMENT_BATCH_AGE_SECS}s"
        ));
    }
    None
}

pub async fn check_settlement(http: &reqwest::Client, _now: UnixSecs) -> Vec<Finding> {
    let fetched: Result<Settlement> = async {
        Ok(http
            .get(SETTLEMENT_URL)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }
    .await;
    let finding = match fetched {
        Ok(s) => Finding::new(
            "main-settlement",
            settlement_problem(&s),
            serde_json::to_string(&s).unwrap_or_default(),
        ),
        Err(e) => Finding::new(
            "main-settlement",
            Some(format!("settlement health fetch failed: {e}")),
            "",
        ),
    };
    vec![finding]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(ts: u64, n: u64) -> Head {
        Head {
            timestamp: ts,
            number: n,
            hash: "0x1".into(),
        }
    }

    #[test]
    fn newest_head_decides() {
        let f = block_finding(&[head(1000, 5), head(2000, 6)], &[], 2100);
        assert_eq!(f.problem, None);
        let f = block_finding(&[head(1000, 5)], &[], 1000 + MAX_BLOCK_AGE_SECS + 1);
        assert!(f.problem.unwrap().starts_with("no Main block"));
    }

    #[test]
    fn no_head_is_a_problem() {
        let f = block_finding(&[], &["a: refused".into()], 0);
        assert!(f.problem.unwrap().contains("a: refused"));
    }

    #[test]
    fn settlement_rules() {
        let ok = Settlement {
            settlement_delay_blocks: Some(10),
            last_batch_age_secs: Some(60),
            ..Default::default()
        };
        assert_eq!(settlement_problem(&ok), None);
        assert!(settlement_problem(&Settlement {
            settlement_delay_blocks: Some(121),
            ..ok.clone()
        })
        .unwrap()
        .contains("delay"));
        assert!(settlement_problem(&Settlement {
            last_batch_age_secs: Some(601),
            ..ok.clone()
        })
        .unwrap()
        .contains("batch age"));
        assert_eq!(
            settlement_problem(&Settlement::default()).unwrap(),
            "settlement checkpoint missing"
        );
    }
}
