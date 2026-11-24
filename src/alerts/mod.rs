// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Infrastructure alerts: Main block production, pool settlement and payouts, bridge transfers, disk
//! usage, pod crashes, Main seed lag, and chain-wide supply, Treasury, DAO and config changes. Each check runs on its own schedule in one thread and reports through the
//! same Telegram bot as the large-transfer alerts.

pub mod bridge;
pub mod chain;
pub mod disk;
pub mod governance;
pub mod kube;
pub mod pods;
pub mod rule;

use crate::telegram::TelegramBot;
use rule::{Action, Alarm, Finding, Rule, UnixSecs};
use std::{collections::HashMap, sync::Arc, thread::JoinHandle, time::Duration};
use tracing::{error, info, warn};

const BLOCK: Rule = Rule {
    every: Duration::from_secs(60),
    after: 1,
    repeat: Duration::from_secs(1200),
};
const SETTLEMENT: Rule = Rule {
    every: Duration::from_secs(60),
    after: 3,
    repeat: Duration::from_secs(1200),
};
const DISK: Rule = Rule {
    every: Duration::from_secs(300),
    after: 1,
    repeat: Duration::from_secs(6 * 3600),
};
const POD: Rule = Rule {
    every: Duration::from_secs(60),
    after: 1,
    repeat: Duration::from_secs(1200),
};
/// Shares POD's schedule; three bad checks ride out a planned seed restart.
const SEED_LAG: Rule = Rule {
    every: Duration::from_secs(60),
    after: 3,
    repeat: Duration::from_secs(1200),
};
const BRIDGE_EVERY: Duration = Duration::from_secs(300);
const GOVERNANCE_EVERY: Duration = Duration::from_secs(120);
const TICK: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Check {
    Block,
    Settlement,
    Disk,
    Bridge,
    Pods,
    Governance,
}

impl Check {
    const ALL: [Check; 6] = [
        Check::Block,
        Check::Settlement,
        Check::Disk,
        Check::Bridge,
        Check::Pods,
        Check::Governance,
    ];

    fn every(self) -> Duration {
        match self {
            Check::Block => BLOCK.every,
            Check::Settlement => SETTLEMENT.every,
            Check::Disk => DISK.every,
            Check::Bridge => BRIDGE_EVERY,
            Check::Pods => POD.every,
            Check::Governance => GOVERNANCE_EVERY,
        }
    }
}

fn now() -> UnixSecs {
    chrono::Utc::now().timestamp() as UnixSecs
}

/// Applies the rule to each finding and returns the messages to send.
fn messages(
    alarms: &mut HashMap<String, Alarm>,
    findings: Vec<Finding>,
    now: UnixSecs,
    rule: &Rule,
) -> Vec<String> {
    let mut out = vec![];
    for f in findings {
        if let Some(p) = &f.problem {
            warn!("alerts | {}: {p} {}", f.key, f.detail);
        }
        let alarm = alarms.entry(f.key.clone()).or_default();
        let (next, action) = alarm.step(f.problem.is_some(), now, rule);
        *alarm = next;
        match action {
            Some(Action::Alert) => out.push(format!(
                "[ALERT] Starcoin {}: {}\n{}",
                f.key,
                f.problem.unwrap_or_default(),
                f.detail
            )),
            Some(Action::Recover) => {
                out.push(format!("[RECOVERY] Starcoin {}\n{}", f.key, f.detail))
            }
            None => {}
        }
    }
    out
}

pub fn run(tg_bot: Arc<TelegramBot>) -> anyhow::Result<JoinHandle<()>> {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?;
    let kube = match kube::Kube::in_cluster() {
        Ok(k) => Some(k),
        Err(e) => {
            warn!("alerts | disk and pod checks disabled until restart: {e}");
            None
        }
    };
    Ok(std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async move {
            info!("alerts | started");
            let mut alarms = HashMap::new();
            let mut bridge = bridge::Bridge::default();
            let mut governance = governance::Governance::default();
            let mut due: HashMap<Check, UnixSecs> = HashMap::new();
            loop {
                let t = now();
                for check in Check::ALL {
                    if due.get(&check).is_some_and(|d| *d > t) {
                        continue;
                    }
                    due.insert(check, t + check.every().as_secs());
                    let msgs = match check {
                        Check::Block => {
                            messages(&mut alarms, chain::check_block(&http, t).await, t, &BLOCK)
                        }
                        Check::Settlement => messages(
                            &mut alarms,
                            chain::check_settlement(&http, t).await,
                            t,
                            &SETTLEMENT,
                        ),
                        Check::Disk => messages(&mut alarms, disk::check(&kube, t).await, t, &DISK),
                        Check::Bridge => bridge.run(&http, t).await,
                        Check::Governance => governance.run(&http, t).await,
                        Check::Pods => {
                            let (pod, lag) = pods::check(&kube, &http, t).await;
                            let mut m = messages(&mut alarms, pod, t, &POD);
                            m.extend(messages(&mut alarms, lag, t, &SEED_LAG));
                            m
                        }
                    };
                    let bad: Vec<_> = alarms
                        .iter()
                        .filter(|(_, a)| a.is_bad())
                        .map(|(k, _)| k.as_str())
                        .collect();
                    info!("alerts | {check:?} done, bad={bad:?}");
                    for m in msgs {
                        if let Err(e) = tg_bot.send_message(&m).await {
                            error!("alerts | telegram failed: {e}; message: {m}");
                        }
                    }
                }
                tokio::time::sleep(TICK).await;
            }
        });
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_and_recovery_messages() {
        let mut alarms = HashMap::new();
        let bad = vec![Finding::new(
            "pvc a/b",
            Some("above 85%".into()),
            "90% used",
        )];
        let m = messages(&mut alarms, bad, 0, &DISK);
        assert_eq!(m, vec!["[ALERT] Starcoin pvc a/b: above 85%\n90% used"]);
        let good = vec![Finding::new("pvc a/b", None, "70% used")];
        let m = messages(&mut alarms, good, 300, &DISK);
        assert_eq!(m, vec!["[RECOVERY] Starcoin pvc a/b\n70% used"]);
    }
}
