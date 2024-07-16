use std::str::FromStr;

use crate::proto::celestia_service_server::CelestiaService as Celestia;
use base64::prelude::*;
use blockscout_display_bytes::Bytes;
use da_indexer_logic::celestia::{l2_router::L2Router, repository::blobs};
use da_indexer_proto::blockscout::da_indexer::v1::{
    CelestiaBlob, CelestiaBlobId, CelestiaL2BatchMetadata, GetCelestiaBlobRequest,
};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct CelestiaService {
    db: DatabaseConnection,
    l2_router: Option<L2Router>,
}

impl CelestiaService {
    pub fn new(db: DatabaseConnection, l2_router: Option<L2Router>) -> Self {
        Self { db, l2_router }
    }
}

#[async_trait::async_trait]
impl Celestia for CelestiaService {
    async fn get_blob(
        &self,
        request: Request<GetCelestiaBlobRequest>,
    ) -> Result<Response<CelestiaBlob>, Status> {
        let inner = request.into_inner();

        let height = inner.height;
        let commitment = Bytes::from_str(&inner.commitment)
            .map(|b| b.to_vec())
            .or_else(|_| BASE64_STANDARD.decode(&inner.commitment))
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to decode commitment");
                Status::invalid_argument("failed to decode commitment")
            })?;

        let blob = blobs::find_by_height_and_commitment(&self.db, height, &commitment)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to query blob");
                Status::internal("failed to query blob")
            })?
            .ok_or(Status::not_found("blob not found"))?;

        let data =
            (!inner.skip_data.unwrap_or_default()).then_some(BASE64_STANDARD.encode(&blob.data));

        Ok(Response::new(CelestiaBlob {
            height: blob.height as u64,
            namespace: hex::encode(blob.namespace),
            commitment: inner.commitment,
            timestamp: blob.timestamp as u64,
            size: blob.data.len() as u64,
            data,
        }))
    }

    async fn get_l2_batch_metadata(
        &self,
        request: Request<CelestiaBlobId>,
    ) -> Result<Response<CelestiaL2BatchMetadata>, Status> {
        match self.l2_router {
            Some(ref l2_router) => {
                let inner = request.into_inner();

                let height = inner.height;
                let commitment = Bytes::from_str(&inner.commitment)
                    .map(|b| b.to_vec())
                    .or_else(|_| BASE64_STANDARD.decode(&inner.commitment))
                    .map_err(|err| {
                        tracing::error!(error = ?err, "failed to decode commitment");
                        Status::invalid_argument("failed to decode commitment")
                    })?;

                let namespace = Bytes::from_str(&inner.namespace)
                    .map(|b| b.to_vec())
                    .or_else(|_| BASE64_STANDARD.decode(&inner.namespace))
                    .map_err(|err| {
                        tracing::error!(error = ?err, "failed to decode commitment");
                        Status::invalid_argument("failed to decode commitment")
                    })?;

                let l2_batch_metadata = l2_router.get_l2_batch_metadata(height, &hex::encode(namespace), &commitment).await.map_err(|err| {
                    tracing::error!(error = ?err, "failed to query l2 batch metadata");
                    Status::internal("failed to query l2 batch metadata")
                })?;

                let related_blobs = l2_batch_metadata.related_blobs.iter().map(|blob| CelestiaBlobId {
                    height: blob.height,
                    namespace: blob.namespace.clone(),
                    commitment: blob.commitment.clone(),
                }).collect();

                Ok(Response::new(CelestiaL2BatchMetadata {
                    chain_id: l2_batch_metadata.chain_id,
                    l2_batch_id: l2_batch_metadata.l2_batch_id,
                    l2_start_block: l2_batch_metadata.l2_start_block,
                    l2_end_block: l2_batch_metadata.l2_end_block,
                    l2_blockscout_url: l2_batch_metadata.l2_blockscout_url,
                    l1_tx_hash: l2_batch_metadata.l1_tx_hash,
                    l1_tx_timestamp: l2_batch_metadata.l1_tx_timestamp ,
                    l2_batch_tx_count: l2_batch_metadata.l2_batch_tx_count,
                    related_blobs,
                }))
            }
            None => Err(Status::unimplemented("l2 router not configured")),
        }
    }
}
