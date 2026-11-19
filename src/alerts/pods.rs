// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Pod crashes in every namespace, and Main seeds falling behind the block-permit signer.
//! Needs `list pods`. On 2026-11-19 both Main seeds crashed (exit 12) and restarted unnoticed
//! for eight hours; the block check alone could not see it because the signer kept producing.

use super::chain::{fetch_head, Head};
use super::kube::Kube;
use super::rule::{Finding, UnixSecs};
use anyhow::Result;
use serde::Deserialize;

/// A container that last exited within this window counts as a fresh crash.
const RESTART_WINDOW_SECS: u64 = 900;
/// About five minutes of Main blocks.
const MAX_SEED_LAG_BLOCKS: u64 = 60;
const MAIN_NAMESPACE: &str = "starcoin-main";
const SEED_APP: &str = "starcoin";
const SIGNER_APP: &str = "starcoin-main-block-permit-signer";
const RPC_PORT: u16 = 9850;
const STUCK_REASONS: [&str; 5] = [
    "CrashLoopBackOff",
    "ImagePullBackOff",
    "ErrImagePull",
    "CreateContainerConfigError",
    "CreateContainerError",
];

#[derive(Debug, Deserialize)]
pub struct PodList {
    pub items: Vec<Pod>,
}

#[derive(Debug, Deserialize)]
pub struct Pod {
    pub metadata: Meta,
    #[serde(default)]
    pub status: Status,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub name: String,
    pub namespace: String,
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    #[serde(default)]
    pub phase: String,
    // Kubernetes spells it `podIP`, which camelCase would turn into `podIp`.
    #[serde(rename = "podIP")]
    pub pod_ip: Option<String>,
    #[serde(default)]
    pub container_statuses: Vec<ContainerStatus>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerStatus {
    pub name: String,
    #[serde(default)]
    pub restart_count: u32,
    #[serde(default)]
    pub state: State,
    #[serde(default)]
    pub last_state: State,
}

#[derive(Debug, Default, Deserialize)]
pub struct State {
    pub waiting: Option<Waiting>,
    pub terminated: Option<Terminated>,
}

#[derive(Debug, Deserialize)]
pub struct Waiting {
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Terminated {
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub exit_code: i32,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

fn container_problem(c: &ContainerStatus, now: UnixSecs) -> Option<String> {
    if let Some(w) = &c.state.waiting {
        if STUCK_REASONS.contains(&w.reason.as_str()) {
            return Some(format!("container {} is {}", c.name, w.reason));
        }
    }
    let t = c.last_state.terminated.as_ref()?;
    let at = t.finished_at?;
    let age = now.saturating_sub(at.timestamp().max(0) as u64);
    (age <= RESTART_WINDOW_SECS).then(|| {
        format!(
            "container {} restarted: {} exit {} at {} (restarts {})",
            c.name,
            t.reason,
            t.exit_code,
            at.format("%H:%M:%S UTC"),
            c.restart_count
        )
    })
}

/// One finding per running pod; completed Job pods are skipped.
pub fn pod_findings(pods: &PodList, now: UnixSecs) -> Vec<Finding> {
    pods.items
        .iter()
        .filter(|p| p.status.phase != "Succeeded")
        .map(|p| {
            let problems: Vec<String> = p
                .status
                .container_statuses
                .iter()
                .filter_map(|c| container_problem(c, now))
                .collect();
            let restarts: u32 = p
                .status
                .container_statuses
                .iter()
                .map(|c| c.restart_count)
                .sum();
            Finding::new(
                format!("pod {}/{}", p.metadata.namespace, p.metadata.name),
                (!problems.is_empty()).then(|| problems.join("; ")),
                format!("phase={} restarts={restarts}", p.status.phase),
            )
        })
        .collect()
}

/// Seed heads compared with the signer head. A seed that cannot answer counts as behind.
pub fn lag_findings(
    signer: &Result<Head, String>,
    seeds: &[(String, Result<Head, String>)],
) -> Vec<Finding> {
    let signer = match signer {
        Ok(h) => h,
        Err(e) => {
            return vec![Finding::new(
                "signer-rpc",
                Some(format!("cannot read the signer head: {e}")),
                "",
            )]
        }
    };
    let mut out = vec![Finding::new(
        "signer-rpc",
        None,
        format!("height={}", signer.number),
    )];
    for (name, head) in seeds {
        let (problem, detail) = match head {
            Ok(h) => {
                let lag = signer.number.saturating_sub(h.number);
                let detail = format!("seed={} signer={} lag={lag}", h.number, signer.number);
                let problem = (lag > MAX_SEED_LAG_BLOCKS)
                    .then(|| format!("{lag} blocks behind the signer (> {MAX_SEED_LAG_BLOCKS})"));
                (problem, detail)
            }
            Err(e) => (Some(format!("RPC failed: {e}")), String::new()),
        };
        out.push(Finding::new(format!("seed {name}"), problem, detail));
    }
    out
}

fn main_pods<'a>(pods: &'a PodList, app: &'a str) -> impl Iterator<Item = &'a Pod> {
    pods.items.iter().filter(move |p| {
        p.metadata.namespace == MAIN_NAMESPACE
            && p.metadata.labels.get("app").map(String::as_str) == Some(app)
    })
}

async fn head_of(http: &reqwest::Client, pod: &Pod) -> Result<Head, String> {
    let ip = pod.status.pod_ip.as_deref().ok_or("no pod IP")?;
    fetch_head(http, &format!("http://{ip}:{RPC_PORT}"))
        .await
        .map_err(|e| e.to_string())
}

/// Returns the pod findings and the seed lag findings; a failed pod list is one finding each.
pub async fn check(
    kube: &Option<Kube>,
    http: &reqwest::Client,
    now: UnixSecs,
) -> (Vec<Finding>, Vec<Finding>) {
    let pods: PodList = match kube {
        Some(k) => match k.get("/api/v1/pods").await {
            Ok(p) => p,
            Err(e) => {
                let f = Finding::new("pods-read", Some(format!("cannot list pods: {e}")), "");
                return (vec![f], vec![]);
            }
        },
        None => {
            let f = Finding::new("pods-read", Some("not running in Kubernetes".into()), "");
            return (vec![f], vec![]);
        }
    };
    let mut pod = pod_findings(&pods, now);
    pod.push(Finding::new("pods-read", None, ""));

    let signer = match main_pods(&pods, SIGNER_APP).next() {
        Some(p) => head_of(http, p).await,
        None => Err("signer pod not found".into()),
    };
    let mut seeds = vec![];
    for p in main_pods(&pods, SEED_APP) {
        seeds.push((p.metadata.name.clone(), head_of(http, p).await));
    }
    (pod, lag_findings(&signer, &seeds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const NOW: UnixSecs = 1795066000; // 2026-11-19T05:26:40Z

    fn pods(v: serde_json::Value) -> PodList {
        serde_json::from_value(json!({ "items": v })).unwrap()
    }

    #[test]
    fn fresh_crash_alerts_and_old_one_does_not() {
        let list = pods(json!([
            {"metadata": {"name": "starcoin-0", "namespace": "starcoin-main"},
             "status": {"phase": "Running", "containerStatuses": [
                {"name": "starcoin", "restartCount": 1, "state": {"running": {}},
                 "lastState": {"terminated": {"reason": "Error", "exitCode": 12,
                                              "finishedAt": "2026-11-19T05:20:35Z"}}}]}},
            {"metadata": {"name": "swap", "namespace": "starcoin-main"},
             "status": {"phase": "Running", "containerStatuses": [
                {"name": "app", "restartCount": 1, "state": {"running": {}},
                 "lastState": {"terminated": {"reason": "Error", "exitCode": 1,
                                              "finishedAt": "2026-10-19T05:20:35Z"}}}]}},
            {"metadata": {"name": "job-x", "namespace": "default"},
             "status": {"phase": "Succeeded"}}
        ]));
        let f = pod_findings(&list, NOW);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].key, "pod starcoin-main/starcoin-0");
        assert_eq!(
            f[0].problem.as_deref(),
            Some("container starcoin restarted: Error exit 12 at 05:20:35 UTC (restarts 1)")
        );
        assert_eq!(f[1].problem, None);
    }

    #[test]
    fn restart_window_edge() {
        let at = |t: &str| {
            pods(json!([
                {"metadata": {"name": "a", "namespace": "n"},
                 "status": {"phase": "Running", "containerStatuses": [
                    {"name": "c", "restartCount": 1, "state": {"running": {}},
                     "lastState": {"terminated": {"reason": "Error", "exitCode": 12,
                                                  "finishedAt": t}}}]}}
            ]))
        };
        // NOW - 900 s is inside the window, NOW - 901 s is not.
        assert!(pod_findings(&at("2026-11-19T05:11:40Z"), NOW)[0]
            .problem
            .is_some());
        assert!(pod_findings(&at("2026-11-19T05:11:39Z"), NOW)[0]
            .problem
            .is_none());
    }

    #[test]
    fn reads_the_pod_ip() {
        let list = pods(json!([
            {"metadata": {"name": "a", "namespace": "n"},
             "status": {"phase": "Running", "podIP": "10.244.2.41"}}
        ]));
        assert_eq!(list.items[0].status.pod_ip.as_deref(), Some("10.244.2.41"));
    }

    #[test]
    fn crash_loop_is_a_problem() {
        let list = pods(json!([
            {"metadata": {"name": "a", "namespace": "n"},
             "status": {"phase": "Running", "containerStatuses": [
                {"name": "c", "restartCount": 5,
                 "state": {"waiting": {"reason": "CrashLoopBackOff"}}}]}}
        ]));
        let f = pod_findings(&list, NOW);
        assert_eq!(
            f[0].problem.as_deref(),
            Some("container c is CrashLoopBackOff")
        );
    }

    fn head(number: u64) -> Head {
        Head {
            timestamp: NOW,
            number,
            hash: String::new(),
        }
    }

    #[test]
    fn seed_lag_threshold_and_rpc_failure() {
        let seeds = vec![
            (
                "starcoin-0".to_string(),
                Ok(head(1000 - MAX_SEED_LAG_BLOCKS)),
            ),
            (
                "starcoin-1".to_string(),
                Ok(head(1000 - MAX_SEED_LAG_BLOCKS - 1)),
            ),
            (
                "starcoin-2".to_string(),
                Err("connection refused".to_string()),
            ),
        ];
        let f = lag_findings(&Ok(head(1000)), &seeds);
        assert_eq!(f[0].key, "signer-rpc");
        assert_eq!(f[1].problem, None);
        assert_eq!(
            f[2].problem.as_deref(),
            Some("61 blocks behind the signer (> 60)")
        );
        assert_eq!(
            f[3].problem.as_deref(),
            Some("RPC failed: connection refused")
        );
        let f = lag_findings(&Err("timeout".into()), &seeds);
        assert_eq!(f.len(), 1);
        assert!(f[0].problem.is_some());
    }
}
