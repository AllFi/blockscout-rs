use serde::Deserialize;
use serde_with::serde_as;

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct IndexerSettings {
    pub disperser: String,
    pub contract_address: String,
    pub rpc: RpcSettings,
    pub start_height: Option<u64>,
}

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RpcSettings {
    pub url: String,
    pub batch_size: u64,
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
            start_height: None,
        }
    }
}
