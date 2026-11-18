// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Disk usage of every node's root disk and every mounted PVC, from each kubelet's
//! /stats/summary read through the API server. Needs `list nodes` and `get nodes/proxy`.

use super::rule::{Finding, UnixSecs};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

const MAX_USED: f64 = 0.85;
const K8S_API: &str = "https://kubernetes.default.svc";
const SA_DIR: &str = "/var/run/secrets/kubernetes.io/serviceaccount";

#[derive(Debug, Deserialize)]
pub struct Summary {
    pub node: NodeStats,
    #[serde(default)]
    pub pods: Vec<PodStats>,
}

#[derive(Debug, Deserialize)]
pub struct NodeStats {
    pub fs: Option<Fs>,
}

#[derive(Debug, Deserialize)]
pub struct PodStats {
    #[serde(default)]
    pub volume: Vec<Volume>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub used_bytes: Option<u64>,
    pub capacity_bytes: Option<u64>,
    pub pvc_ref: Option<PvcRef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fs {
    pub used_bytes: Option<u64>,
    pub capacity_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct PvcRef {
    pub name: String,
    pub namespace: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    pub used: u64,
    pub capacity: u64,
}

/// One node's summary -> usage by key, for the root disk and each mounted PVC.
pub fn usage(node: &str, s: &Summary) -> BTreeMap<String, Usage> {
    let mut out = BTreeMap::new();
    if let Some(Fs {
        used_bytes: Some(used),
        capacity_bytes: Some(capacity),
    }) = s.node.fs
    {
        if capacity > 0 {
            out.insert(format!("node {node} root disk"), Usage { used, capacity });
        }
    }
    for v in s.pods.iter().flat_map(|p| &p.volume) {
        if let (Some(r), Some(used), Some(capacity)) = (&v.pvc_ref, v.used_bytes, v.capacity_bytes)
        {
            if capacity > 0 {
                out.insert(
                    format!("pvc {}/{}", r.namespace, r.name),
                    Usage { used, capacity },
                );
            }
        }
    }
    out
}

pub fn findings(usage: &BTreeMap<String, Usage>) -> Vec<Finding> {
    usage
        .iter()
        .map(|(key, u)| {
            let share = u.used as f64 / u.capacity as f64;
            let gb = |b: u64| b as f64 / 1e9;
            let detail = format!(
                "{:.0}% used, {:.0}G of {:.0}G, {:.0}G free",
                share * 100.0,
                gb(u.used),
                gb(u.capacity),
                gb(u.capacity - u.used.min(u.capacity))
            );
            let problem = (share >= MAX_USED).then(|| format!("above {:.0}%", MAX_USED * 100.0));
            Finding::new(key.clone(), problem, detail)
        })
        .collect()
}

/// API server client using the pod's service account.
pub struct Kube {
    http: reqwest::Client,
}

impl Kube {
    pub fn in_cluster() -> Result<Self> {
        let ca = std::fs::read(format!("{SA_DIR}/ca.crt")).context("read service account CA")?;
        let http = reqwest::Client::builder()
            .add_root_certificate(reqwest::Certificate::from_pem(&ca)?)
            .timeout(std::time::Duration::from_secs(20))
            .build()?;
        Ok(Self { http })
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        // The projected token rotates; read it for every request.
        let token = std::fs::read_to_string(format!("{SA_DIR}/token"))
            .context("read service account token")?;
        let resp = self
            .http
            .get(format!("{K8S_API}{path}"))
            .bearer_auth(token.trim())
            .send()
            .await?;
        Ok(resp.error_for_status()?.json().await?)
    }

    async fn all_usage(&self) -> Result<BTreeMap<String, Usage>> {
        #[derive(Deserialize)]
        struct Nodes {
            items: Vec<Item>,
        }
        #[derive(Deserialize)]
        struct Item {
            metadata: Meta,
        }
        #[derive(Deserialize)]
        struct Meta {
            name: String,
        }
        let nodes: Nodes = self.get("/api/v1/nodes").await?;
        let mut out = BTreeMap::new();
        for n in nodes.items {
            let name = n.metadata.name;
            let s: Summary = self
                .get(&format!("/api/v1/nodes/{name}/proxy/stats/summary"))
                .await?;
            out.extend(usage(&name, &s));
        }
        Ok(out)
    }
}

pub async fn check(kube: &Option<Kube>, _now: UnixSecs) -> Vec<Finding> {
    let read = match kube {
        Some(k) => k.all_usage().await,
        None => Err(anyhow::anyhow!(
            "not running in Kubernetes (no service account)"
        )),
    };
    match read {
        Ok(u) => {
            let mut out = findings(&u);
            out.push(Finding::new("disk-read", None, ""));
            out
        }
        Err(e) => vec![Finding::new(
            "disk-read",
            Some(format!("cannot read disk usage: {e}")),
            "",
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1_000_000_000;

    #[test]
    fn root_disk_and_pvcs_from_kubelet_summary() {
        let s: Summary = serde_json::from_value(serde_json::json!({
            "node": {"nodeName": "n1", "fs": {"usedBytes": 14 * GB, "capacityBytes": 160 * GB}},
            "pods": [
                {"volume": [
                    {"name": "data", "usedBytes": 520 * GB, "capacityBytes": 600 * GB,
                     "pvcRef": {"name": "es-data-600", "namespace": "default"}},
                    {"name": "kube-api-access", "usedBytes": 1, "capacityBytes": 10}
                ]},
                {}
            ]
        }))
        .unwrap();
        let u = usage("n1", &s);
        assert_eq!(
            u.keys().collect::<Vec<_>>(),
            vec!["node n1 root disk", "pvc default/es-data-600"]
        );
        let f = findings(&u);
        assert_eq!(f[0].problem, None);
        assert_eq!(f[1].problem.as_deref(), Some("above 85%"));
        assert!(f[1].detail.starts_with("87% used"));
    }

    #[test]
    fn threshold_is_inclusive() {
        let one = |used| {
            BTreeMap::from([(
                "k".to_string(),
                Usage {
                    used,
                    capacity: 100,
                },
            )])
        };
        assert!(findings(&one(85))[0].problem.is_some());
        assert!(findings(&one(84))[0].problem.is_none());
    }
}
