// Copyright (c) The Starcoin Core Contributors
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub starcoin_rpc_url: String,
    pub telegram_bot_token: String,
    pub telegram_chat_id: String,
    pub telegram_proxy: Option<String>,
    pub min_transaction_amount: u128,
    pub es_url: String,
    pub es_user_name: String,
    pub es_password: String,
}

impl Config {
    /// Reads `.env` and the process environment once, at the process boundary.
    pub fn load() -> Result<Self> {
        dotenv::dotenv().ok();
        Self::from_vars(|name| match env::var(name) {
            Ok(value) => Ok(Some(value)),
            Err(env::VarError::NotPresent) => Ok(None),
            Err(e) => Err(anyhow!("{name}: {e}")),
        })
    }

    /// Builds the config from a variable lookup, so parsing is testable without the process env.
    pub fn from_vars(var: impl Fn(&str) -> Result<Option<String>>) -> Result<Self> {
        let required = |name: &str| var(name)?.ok_or_else(|| anyhow!("{name} is not set"));
        let or_default = |name: &str, default: &str| -> Result<String> {
            Ok(var(name)?.unwrap_or_else(|| default.to_string()))
        };
        let min_transaction_amount = or_default("MIN_TRANSACTION_AMOUNT", "1000000000")?;
        Ok(Config {
            starcoin_rpc_url: or_default("STARCOIN_RPC_URL", "ws://main.seed.starcoin.org:9870")?,
            telegram_bot_token: required("TELEGRAM_BOT_TOKEN")?,
            telegram_chat_id: required("TELEGRAM_CHAT_ID")?,
            telegram_proxy: var("TELOXIDE_PROXY")?,
            min_transaction_amount: min_transaction_amount.parse().with_context(|| {
                format!("MIN_TRANSACTION_AMOUNT is not a u128: {min_transaction_amount}")
            })?,
            es_url: or_default("ES_URL", "http://127.0.0.1:9200")?,
            es_user_name: or_default("ES_USER_NAME", "elastic")?,
            es_password: or_default("ES_PASSWORD", "changeme")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config_from(pairs: &[(&str, &str)]) -> Result<Config> {
        let vars: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_vars(|name| Ok(vars.get(name).cloned()))
    }

    const TELEGRAM: [(&str, &str); 2] =
        [("TELEGRAM_BOT_TOKEN", "token"), ("TELEGRAM_CHAT_ID", "42")];

    #[test]
    fn test_config_default_es_url() {
        let config = config_from(&TELEGRAM).unwrap();
        assert_eq!(config.es_url, "http://127.0.0.1:9200");
        assert_eq!(config.es_user_name, "elastic");
        assert_eq!(config.es_password, "changeme");
        assert_eq!(config.min_transaction_amount, 1_000_000_000);
        assert_eq!(config.telegram_proxy, None);
    }

    #[test]
    fn test_config_custom_es_url() {
        let mut pairs = TELEGRAM.to_vec();
        pairs.extend([
            ("ES_URL", "http://localhost:9200"),
            ("ES_USER_NAME", "admin"),
            ("ES_PASSWORD", "secret"),
            ("MIN_TRANSACTION_AMOUNT", "500000000000000"),
        ]);
        let config = config_from(&pairs).unwrap();
        assert_eq!(config.es_url, "http://localhost:9200");
        assert_eq!(config.es_user_name, "admin");
        assert_eq!(config.es_password, "secret");
        assert_eq!(config.min_transaction_amount, 500_000_000_000_000);
    }

    #[test]
    fn missing_telegram_token_is_an_error() {
        let err = config_from(&[("TELEGRAM_CHAT_ID", "42")]).unwrap_err();
        assert!(err.to_string().contains("TELEGRAM_BOT_TOKEN"), "{err}");
    }

    #[test]
    fn unparsable_min_amount_is_an_error() {
        let mut pairs = TELEGRAM.to_vec();
        pairs.push(("MIN_TRANSACTION_AMOUNT", "5e14"));
        let err = config_from(&pairs).unwrap_err();
        assert!(err.to_string().contains("MIN_TRANSACTION_AMOUNT"), "{err}");
    }
}
