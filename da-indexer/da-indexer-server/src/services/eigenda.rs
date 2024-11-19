use crate::{proto::eigen_da_service_server::EigenDaService as EigenDa, Settings};
use base64::prelude::*;
use da_indexer_logic::{
    common::eth_provider::EthProvider,
    eigenda::{
        client::Client,
        job::EigenDAJob,
        repository::blobs::{self, Blob},
    },
    settings::DASettings,
};
use da_indexer_proto::blockscout::da_indexer::v1::{EigenDaBlob, GetEigenDaBlobRequest};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

use super::bytes_from_hex_or_base64;

#[derive(Default)]
pub struct EigenDaService {
    db: Option<DatabaseConnection>,
    client: Option<Client>,
    eth: Option<EthProvider>,
    eigenda_address: String,
}

impl EigenDaService {
    pub async fn new(db: Option<DatabaseConnection>, settings: &Settings) -> anyhow::Result<Self> {
        let (client, eth, eigenda_address) = match &settings.indexer {
            Some(indexer) => match &indexer.da {
                DASettings::EigenDA(da) => (
                    Some(Client::new(&da.disperser_url, vec![0]).await?),
                    Some(EthProvider::new(&da.rpc.url).await?),
                    da.eigenda_address.clone(),
                ),
                _ => (None, None, String::new()),
            },
            None => (None, None, String::new()),
        };
        Ok(Self {
            db,
            client,
            eth,
            eigenda_address,
        })
    }

    async fn fetch_blob_directly(
        &self,
        batch_header_hash: Vec<u8>,
        blob_index: u32,
    ) -> Result<Option<Blob>, anyhow::Error> {
        let eth = self
            .eth
            .as_ref()
            .ok_or(anyhow::anyhow!("client is not configured"))?;
        let client = self
            .client
            .as_ref()
            .ok_or(anyhow::anyhow!("client is not configured"))?;
        let last_block = eth.get_block_number().await?;
        let jobs = eth
            .get_logs(
                &self.eigenda_address,
                "BatchConfirmed(bytes32,uint32)",
                last_block - 1000,
                last_block,
                1000,
                None,
            )
            .await?
            .into_iter()
            .filter_map(|log| EigenDAJob::try_from(log).ok())
            .collect::<Vec<_>>();

        for job in jobs {
            if job.batch_header_hash == batch_header_hash.as_slice() {
                let blob = client
                    .retrieve_blob_with_retries(job.batch_id, &batch_header_hash, blob_index as u32)
                    .await;
                return blob.map(|blob| {
                    blob.map(|data| Blob {
                        batch_id: job.batch_id as i64,
                        batch_header_hash: batch_header_hash.clone(),
                        blob_index: blob_index as i32,
                        l1_block: job.block_number as i64,
                        l1_tx_hash: job.tx_hash.as_bytes().to_vec(),
                        data,
                    })
                });
            }
        }

        Ok(None)
    }
}

#[async_trait::async_trait]
impl EigenDa for EigenDaService {
    async fn get_blob(
        &self,
        request: Request<GetEigenDaBlobRequest>,
    ) -> Result<Response<EigenDaBlob>, Status> {
        let db = self
            .db
            .as_ref()
            .ok_or(Status::unimplemented("database is not configured"))?;
        let inner = request.into_inner();

        let blob_index = inner.blob_index;
        let batch_header_hash =
            bytes_from_hex_or_base64(&inner.batch_header_hash, "batch header hash")?;

        let blob = blobs::find(db, &batch_header_hash, blob_index as i32)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to query blob");
                Status::internal("failed to query blob")
            })?;

        let blob = match blob {
            Some(blob) => blob,
            None => self
                .fetch_blob_directly(batch_header_hash, blob_index)
                .await
                .ok()
                .flatten()
                .ok_or(Status::not_found("blob not found"))?,
        };

        let data =
            (!inner.skip_data.unwrap_or_default()).then_some(BASE64_STANDARD.encode(&blob.data));

        Ok(Response::new(EigenDaBlob {
            batch_header_hash: inner.batch_header_hash,
            batch_id: blob.batch_id as u64,
            blob_index: blob.blob_index as u32,
            l1_confirmation_block: blob.l1_block as u64,
            l1_confirmation_tx_hash: format!("0x{}", hex::encode(blob.l1_tx_hash)),
            size: blob.data.len() as u64,
            data,
        }))
    }
}
