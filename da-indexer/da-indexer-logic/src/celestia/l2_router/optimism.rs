use super::{types::L2BatchMetadata, L2Config};
use anyhow::Result;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
struct Blob {
    commitment: String,
    height: u64,
    l1_timestamp: String,
    l1_transaction_hash: String,
    namespace: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct L2BatchOptimism {
    batch_data_container: String,
    blobs: Vec<Blob>,
    internal_id: u64,
    l1_timestamp: String,
    l1_tx_hashes: Vec<String>,
    l2_block_start: u64,
    l2_block_end: u64,
    tx_count: u64,
}

pub async fn get_l2_batch_optimism(
    config: &L2Config,
    height: u64,
    commitment: &[u8],
) -> Result<L2BatchMetadata> {
    let query = format!(
        "{}/api/v2/optimism/batches/da/celestia/{}/{}",
        config.l2_api_url,
        height,
        format!("0x{}", hex::encode(commitment)),
    );
    let response: L2BatchOptimism = reqwest::get(&query).await?.json().await?;
    parse_l2_batch_metadata(commitment, &response, config)
}

pub fn parse_l2_batch_metadata(
    commitment: &[u8],
    response: &L2BatchOptimism,
    config: &L2Config,
) -> Result<L2BatchMetadata> {
    let related_blobs = response
        .blobs
        .iter()
        .filter(|blob| blob.commitment.trim_start_matches("0x") != hex::encode(commitment))
        .map(|blob| super::types::CelestiaBlobId {
            namespace: blob.namespace.clone(),
            height: blob.height,
            commitment: blob.commitment.clone(),
        })
        .collect();

    Ok(L2BatchMetadata {
        chain_type: super::types::L2Type::Optimism,
        chain_id: config.chain_id,
        l2_batch_id: response.internal_id.to_string(),
        l2_start_block: response.l2_block_start,
        l2_end_block: response.l2_block_end,
        l2_batch_tx_count: response.tx_count as u32,
        l2_blockscout_url: config.l2_blockscout_url.clone(),
        l1_tx_hash: response.l1_tx_hashes[0].clone(),
        l1_tx_timestamp: DateTime::parse_from_rfc3339(&response.l1_timestamp)?.timestamp() as u64, // TODO: incorrect
        related_blobs,
    })
}
