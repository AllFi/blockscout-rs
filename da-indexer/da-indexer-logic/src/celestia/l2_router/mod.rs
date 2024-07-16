mod optimism;
pub mod types;

use anyhow::{anyhow, Result};
use optimism::get_l2_batch_optimism;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use types::{L2BatchMetadata, L2Config, L2Type};

#[derive(Serialize, Deserialize)]
pub struct L2Router {
    pub routes: HashMap<String, L2Config>,
}

impl L2Router {
    pub fn new(routes: HashMap<String, L2Config>) -> Result<Self> {
        Ok(Self { routes })
    }

    pub fn parse_from_config(config: &str) -> Result<Self> {
        // let routes: HashMap<String, L2Config> = serde_json::from_str(config)?;
        // Ok(Self::new(routes)?)
        todo!()
    }

    pub async fn get_l2_batch_metadata(
        &self,
        height: u64,
        namespace: &str,
        commitment: &[u8],
    ) -> Result<L2BatchMetadata> {
        let config = self
            .routes
            .get(namespace)
            .ok_or_else(|| anyhow!("no config found for namespace {}", namespace))?;
        match config.chain_type {
            L2Type::Optimism => get_l2_batch_optimism(config, height, commitment).await,
            _ => Err(anyhow!("unsupported chain type: {:?}", config.chain_type)),
        }
    }
}
