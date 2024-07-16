use std::fs;

use anyhow::Result;
use da_indexer_logic::celestia::l2_router::L2Router;

pub async fn create_l2_router(l2_router_config: String) -> Result<L2Router> {
    let routes = fs::read_to_string(l2_router_config)?;
    Ok(toml::from_str(&routes)?)
}
