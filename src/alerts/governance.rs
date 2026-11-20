// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Chain-wide value and rules on Main: STC supply, the Treasury, DAO proposals, framework and
//! on-chain config, and the upgrade settings of the contracts we run. A change in any of these
//! moves value or rules for everyone, so each one is reported once when it happens.

use super::rule::UnixSecs;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const MAIN_RPC_URL: &str = "http://starcoin-svc.starcoin-main.svc.cluster.local";

/// `Dao::DaoGlobalInfo<STC>.next_proposal_id` on 2026-11-20 (proposals 0-13 exist). A higher
/// value means a proposal we have not reviewed, even one created while the monitor was down.
/// Raise this after reviewing a new proposal.
const KNOWN_NEXT_PROPOSAL_ID: u64 = 14;

/// STC's mint capability was destroyed at genesis; block rewards come out of the Treasury. The
/// supply has been 3,185,136,000 STC (9 decimals) since, and any change is a mint or a burn.
const STC_SUPPLY: u128 = 3_185_136_000_000_000_000;

/// Treasury outflow allowed per block on top of the block reward (uncle rewards and rounding).
const TREASURY_SLACK_PER_BLOCK: u128 = 5_000_000_000;

/// Resources whose any change is a rule change: (label, address, type).
const WATCHED: [(&str, &str, &str); 10] = [
    (
        "framework version",
        "0x1",
        "0x1::Config::Config<0x1::Version::Version>",
    ),
    (
        "DAO config",
        "0x1",
        "0x1::Config::Config<0x1::Dao::DaoConfig<0x1::STC::STC>>",
    ),
    (
        "module publishing / script option",
        "0x1",
        "0x1::Config::Config<0x1::TransactionPublishOption::TransactionPublishOption>",
    ),
    (
        "VM config",
        "0x1",
        "0x1::Config::Config<0x1::VMConfig::VMConfig>",
    ),
    (
        "consensus config",
        "0x1",
        "0x1::Config::Config<0x1::ConsensusConfig::ConsensusConfig>",
    ),
    (
        "framework upgrade plan",
        "0x1",
        "0x1::PackageTxnManager::TwoPhaseUpgradeV2",
    ),
    (
        "Starswap upgrade plan",
        "0x8c109349c6bd91411d6bc962e080c4a3",
        "0x1::PackageTxnManager::TwoPhaseUpgradeV2",
    ),
    (
        "Starswap upgrade strategy",
        "0x8c109349c6bd91411d6bc962e080c4a3",
        "0x1::PackageTxnManager::ModuleUpgradeStrategy",
    ),
    (
        "bridge upgrade strategy",
        "0x88d4db8660b9a4c5d91b8d0b40c578e2",
        "0x1::PackageTxnManager::ModuleUpgradeStrategy",
    ),
    (
        "bridge upgrade plan",
        "0x88d4db8660b9a4c5d91b8d0b40c578e2",
        "0x1::PackageTxnManager::TwoPhaseUpgradeV2",
    ),
];

/// What one poll reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub head: u64,
    pub stc_supply: u128,
    pub stc_mints: u64,
    pub stc_burns: u64,
    pub treasury: u128,
    pub reward_per_block: u128,
    pub next_proposal_id: u64,
    /// Label -> the resource's JSON, `null` when absent.
    pub watched: BTreeMap<&'static str, String>,
}

fn stc(nano: u128) -> String {
    format!("{} STC", nano / 1_000_000_000)
}

/// Messages for what changed between two polls; `prev` is `None` on the first poll.
pub fn changes(prev: Option<&Snapshot>, cur: &Snapshot) -> Vec<String> {
    let mut out = vec![];
    let seen_proposals = prev.map_or(KNOWN_NEXT_PROPOSAL_ID, |p| p.next_proposal_id);
    if cur.next_proposal_id > seen_proposals {
        out.push(format!(
            "[ALERT] Starcoin DAO: new proposal id {}..{}\nVoting opens after 1h and lasts 7 \
             days; it can execute 24h after passing. Framework v12 lets votes be counted twice, \
             so against-votes alone may not stop it: review it now, and if it is not ours, plan \
             to keep its action out of blocks.",
            seen_proposals,
            cur.next_proposal_id - 1
        ));
    }
    // Against the known supply on the first poll, then against the previous poll, so one
    // change is reported once and a later legitimate burn does not alert forever.
    let expected_supply = prev.map_or(STC_SUPPLY, |p| p.stc_supply);
    if cur.stc_supply != expected_supply {
        out.push(format!(
            "[ALERT] Starcoin STC supply is {} (was {})",
            stc(cur.stc_supply),
            stc(expected_supply)
        ));
    }
    if let Some(p) = prev {
        if (p.stc_mints, p.stc_burns) != (cur.stc_mints, cur.stc_burns) {
            out.push(format!(
                "[ALERT] Starcoin STC mint/burn events moved: mints {} -> {}, burns {} -> {}",
                p.stc_mints, cur.stc_mints, p.stc_burns, cur.stc_burns
            ));
        }
        let blocks = u128::from(cur.head.saturating_sub(p.head));
        let allowed = blocks * (cur.reward_per_block + TREASURY_SLACK_PER_BLOCK);
        let outflow = p.treasury.saturating_sub(cur.treasury);
        if outflow > allowed {
            out.push(format!(
                "[ALERT] Starcoin Treasury paid out {} over {} blocks (block rewards allow {}); \
                 now {}",
                stc(outflow),
                blocks,
                stc(allowed),
                stc(cur.treasury)
            ));
        }
        for (label, value) in &cur.watched {
            if p.watched.get(label) != Some(value) {
                out.push(format!(
                    "[ALERT] Starcoin {label} changed\nwas: {}\nnow: {value}",
                    p.watched.get(label).map_or("?", String::as_str)
                ));
            }
        }
    }
    out
}

async fn call(http: &reqwest::Client, method: &str, params: Value) -> Result<Value> {
    let body = json!({"id": 1, "jsonrpc": "2.0", "method": method, "params": params});
    let v: Value = http
        .post(MAIN_RPC_URL)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if let Some(e) = v.get("error") {
        bail!("{method}: {e}");
    }
    Ok(v["result"].clone())
}

async fn resource(http: &reqwest::Client, addr: &str, ty: &str) -> Result<Value> {
    let r = call(
        http,
        "state.get_resource",
        json!([addr, ty, {"decode": true}]),
    )
    .await
    .with_context(|| format!("{addr} {ty}"))?;
    Ok(r.get("json").cloned().unwrap_or(Value::Null))
}

fn int(v: &Value, path: &[&str]) -> Result<u128> {
    let mut cur = v;
    for k in path {
        cur = &cur[*k];
    }
    cur.as_u64()
        .map(u128::from)
        .or_else(|| cur.as_str().and_then(|s| s.parse().ok()))
        .with_context(|| format!("{} in {}", path.join("."), v))
}

pub async fn fetch(http: &reqwest::Client) -> Result<Snapshot> {
    let head = call(http, "chain.info", json!([])).await?;
    let head = int(&head, &["head", "number"])? as u64;
    let token = resource(http, "0x1", "0x1::Token::TokenInfo<0x1::STC::STC>").await?;
    let treasury = resource(http, "0x1", "0x1::Treasury::Treasury<0x1::STC::STC>").await?;
    let epoch = resource(http, "0x1", "0x1::Epoch::Epoch").await?;
    let dao = resource(http, "0x1", "0x1::Dao::DaoGlobalInfo<0x1::STC::STC>").await?;
    let mut watched = BTreeMap::new();
    for (label, addr, ty) in WATCHED {
        watched.insert(label, resource(http, addr, ty).await?.to_string());
    }
    Ok(Snapshot {
        head,
        stc_supply: int(&token, &["total_value"])?,
        stc_mints: int(&token, &["mint_events", "counter"])? as u64,
        stc_burns: int(&token, &["burn_events", "counter"])? as u64,
        treasury: int(&treasury, &["balance", "value"])?,
        reward_per_block: int(&epoch, &["reward_per_block"])?,
        next_proposal_id: int(&dao, &["next_proposal_id"])? as u64,
        watched,
    })
}

const ERROR_REPEAT_SECS: u64 = 3600;

#[derive(Default)]
pub struct Governance {
    last: Option<Snapshot>,
    last_error: Option<UnixSecs>,
}

impl Governance {
    pub async fn run(&mut self, http: &reqwest::Client, now: UnixSecs) -> Vec<String> {
        match fetch(http).await {
            Ok(cur) => {
                let msgs = changes(self.last.as_ref(), &cur);
                tracing::info!(
                    "governance | head={} supply={} treasury={} next_proposal={} alerts={}",
                    cur.head,
                    cur.stc_supply,
                    cur.treasury,
                    cur.next_proposal_id,
                    msgs.len()
                );
                self.last = Some(cur);
                self.last_error = None;
                msgs
            }
            Err(e) => {
                tracing::error!("governance | {e:#}");
                let due = self
                    .last_error
                    .is_none_or(|at| now.saturating_sub(at) >= ERROR_REPEAT_SECS);
                if !due {
                    return vec![];
                }
                self.last_error = Some(now);
                vec![format!(
                    "[ALERT] Starcoin governance watch cannot read Main state\n{e:#}"
                )]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            head: 1000,
            stc_supply: STC_SUPPLY,
            stc_mints: 1,
            stc_burns: 0,
            treasury: 2_767_750_510_658_600_000,
            reward_per_block: 5_000_000_000,
            next_proposal_id: KNOWN_NEXT_PROPOSAL_ID,
            watched: WATCHED
                .iter()
                .map(|(l, _, _)| (*l, "{}".to_string()))
                .collect(),
        }
    }

    #[test]
    fn quiet_chain_is_quiet() {
        let a = snap();
        let mut b = snap();
        b.head += 100;
        b.treasury -= 100 * 5_000_000_000;
        assert!(changes(None, &a).is_empty());
        assert!(changes(Some(&a), &b).is_empty());
    }

    #[test]
    fn unreviewed_proposal_alerts_even_on_first_poll() {
        let mut a = snap();
        a.next_proposal_id += 1;
        let m = changes(None, &a);
        assert_eq!(m.len(), 1);
        assert!(m[0].contains("new proposal id 14..14"), "{}", m[0]);
        assert!(changes(Some(&a), &a).is_empty());
    }

    #[test]
    fn supply_change_alerts_once() {
        let a = snap();
        let mut b = snap();
        b.stc_supply -= 1;
        assert_eq!(changes(Some(&a), &b).len(), 1);
        assert!(changes(Some(&b), &b).is_empty());
    }

    #[test]
    fn supply_and_mint_changes_alert() {
        let a = snap();
        let mut b = snap();
        b.stc_supply += 1;
        b.stc_mints += 1;
        let m = changes(Some(&a), &b);
        assert!(m.iter().any(|s| s.contains("STC supply")));
        assert!(m.iter().any(|s| s.contains("mint/burn")));
    }

    #[test]
    fn treasury_outflow_beyond_rewards_alerts() {
        let a = snap();
        let mut b = snap();
        b.head += 10;
        b.treasury -= 1_000_000 * 1_000_000_000;
        let m = changes(Some(&a), &b);
        assert_eq!(m.len(), 1);
        assert!(
            m[0].contains("Treasury paid out 1000000 STC over 10 blocks"),
            "{}",
            m[0]
        );
    }

    #[test]
    fn watched_change_alerts_with_both_values() {
        let a = snap();
        let mut b = snap();
        b.watched
            .insert("module publishing / script option", "{\"x\":1}".to_string());
        let m = changes(Some(&a), &b);
        assert_eq!(m.len(), 1);
        assert!(m[0].contains("module publishing / script option changed"));
        assert!(m[0].contains("now: {\"x\":1}"));
    }

    #[test]
    fn numbers_parse_from_rpc_shapes() {
        let v = json!({"a": {"b": 5}, "s": "3185136000000000000", "f": 2.7e19});
        assert_eq!(int(&v, &["a", "b"]).unwrap(), 5);
        assert_eq!(int(&v, &["s"]).unwrap(), 3_185_136_000_000_000_000);
        // A float means the value lost precision on the way; refuse it.
        assert!(int(&v, &["f"]).is_err());
        assert!(int(&v, &["missing"]).is_err());
    }
}
