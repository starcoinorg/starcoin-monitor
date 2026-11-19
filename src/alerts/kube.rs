// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Read-only Kubernetes API client using the pod's service account.

use anyhow::{Context, Result};

const K8S_API: &str = "https://kubernetes.default.svc";
const SA_DIR: &str = "/var/run/secrets/kubernetes.io/serviceaccount";

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

    pub async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        // The projected token rotates; read it for every request.
        let token = std::fs::read_to_string(format!("{SA_DIR}/token"))
            .context("read service account token")?;
        let resp = self
            .http
            .get(format!("{K8S_API}{path}"))
            .bearer_auth(token.trim())
            .send()
            .await
            .with_context(|| format!("GET {path}"))?;
        Ok(resp.error_for_status()?.json().await?)
    }
}
