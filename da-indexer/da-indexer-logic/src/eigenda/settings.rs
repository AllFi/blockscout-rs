use std::time;

use serde::Deserialize;
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    pub disperser: String,
    pub contract_address: String,
    pub rpc: RpcSettings,
    pub concurrency: u32,
    pub start_height: Option<u64>,
    #[serde(default = "default_restart_delay")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub restart_delay: time::Duration,
    #[serde(default = "default_polling_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub polling_interval: time::Duration,
    #[serde(default = "default_retry_interval")]
    #[serde_as(as = "serde_with::DurationSeconds<u64>")]
    pub retry_interval: time::Duration,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RpcSettings {
    pub url: String,
    pub batch_size: u64,
}

fn default_polling_interval() -> time::Duration {
    time::Duration::from_secs(0)
}

fn default_retry_interval() -> time::Duration {
    time::Duration::from_secs(180)
}

fn default_restart_delay() -> time::Duration {
    time::Duration::from_secs(60)
}

impl Default for IndexerSettings {
    fn default() -> Self {
        Self {
            disperser: "https://disperser-holesky.eigenda.xyz:443".to_string(),
            contract_address: "0xD4A7E1Bd8015057293f0D0A557088c286942e84b".to_string(),
            rpc: RpcSettings {
                url: "https://holesky.drpc.org".to_string(),
                batch_size: 10000,
            },
            concurrency: 2,
            start_height: None,
            restart_delay: default_restart_delay(),
            polling_interval: default_polling_interval(),
            retry_interval: default_retry_interval(),
        }
    }
}