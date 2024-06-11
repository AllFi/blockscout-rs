use anyhow::Result;
use async_trait::async_trait;
use std::{
    cmp::min,
    sync::{atomic::AtomicU64, Arc},
};
use tokio::sync::RwLock;

use sea_orm::DatabaseConnection;

use crate::{
    common::eth_provider::EthProvider,
    eigenda::repository::{batches, blobs},
    indexer::{Job, DA},
};

use super::{client::Client, job::EigenDAJob, repository::batches::Gap, settings::IndexerSettings};

pub struct EigenDA {
    settings: IndexerSettings,

    db: Arc<DatabaseConnection>,
    client: Client,
    provider: EthProvider,

    last_known_block: AtomicU64,
    unprocessed_gaps: RwLock<Vec<Gap>>,
}

impl EigenDA {
    pub async fn new(db: Arc<DatabaseConnection>, settings: IndexerSettings) -> Result<Self> {
        let provider = EthProvider::new(settings.rpc.url.clone()).await?;
        // TODO: add retry delays to settings
        let client = Client::new(settings.disperser.clone(), vec![1, 3, 5, 10, 15]);
        let start_from = settings
            .start_height
            .unwrap_or(provider.get_block_number().await?);
        let gaps = batches::find_gaps(
            &db,
            settings.contract_creation_block as i64,
            start_from as i64,
        )
        .await?;
        Ok(Self {
            settings: settings.clone(),
            db,
            client,
            provider,
            last_known_block: AtomicU64::new(start_from.saturating_sub(1)), // TODO: check it
            unprocessed_gaps: RwLock::new(gaps),
        })
    }

    async fn jobs_from_block_range(
        &self,
        from: u64,
        to: u64,
        soft_limit: Option<u64>,
    ) -> Result<Vec<Job>> {
        let jobs = self
            .provider
            .get_logs(
                &self.settings.contract_address,
                "BatchConfirmed(bytes32,uint32)",
                from,
                to,
                self.settings.rpc.batch_size,
                soft_limit,
            )
            .await?
            .into_iter()
            .filter_map(|log| EigenDAJob::try_from(log).ok().map(Job::EigenDA))
            .collect();
        Ok(jobs)
    }
}

#[async_trait]
impl DA for EigenDA {
    async fn process_job(&self, job: Job) -> Result<()> {
        let job = EigenDAJob::from(job);
        tracing::info!(tx_hash = ?job.tx_hash, batch_header_hash = hex::encode(&job.batch_header_hash), "processing job");
        let blobs = self
            .client
            .retrieve_blobs(job.batch_header_hash.clone())
            .await?;
        tracing::info!(count = blobs.len(), "retrieved blobs");

        let blobs_len = blobs.len();
        if !blobs.is_empty() {
            let chunk_size = 50;
            for (chunk_index, chunk) in blobs.chunks(chunk_size).enumerate() {
                let start_index = chunk_index * chunk_size;
                blobs::upsert_many(
                    self.db.as_ref(),
                    start_index as i32,
                    &job.batch_header_hash,
                    chunk.to_vec(),
                )
                .await?;
            }
        }

        batches::upsert(
            self.db.as_ref(),
            &job.batch_header_hash,
            job.batch_id as i64,
            blobs_len as i32,
            job.tx_hash.as_bytes(),
            job.block_number as i64,
        )
        .await?;

        Ok(())
    }

    async fn new_jobs(&self) -> Result<Vec<Job>> {
        let last_block = self.provider.get_block_number().await?;
        let from = self
            .last_known_block
            .load(std::sync::atomic::Ordering::Relaxed)
            + 1;
        let to = min(from + self.settings.rpc.batch_size, last_block);
        if to < from {
            return Ok(vec![]);
        }

        let jobs = self.jobs_from_block_range(from, to, None).await?;
        self.last_known_block
            .store(to, std::sync::atomic::Ordering::Relaxed);
        Ok(jobs)
    }

    async fn unprocessed_jobs(&self) -> Result<Vec<Job>> {
        let mut jobs = vec![];
        let mut new_gaps = vec![];
        let mut unprocessed_gaps = self.unprocessed_gaps.write().await;
        tracing::info!("gaps: {:?}", unprocessed_gaps);
        for gap in unprocessed_gaps.iter() {
            if !jobs.is_empty() {
                new_gaps.push(gap.clone());
                continue;
            }

            let from = gap.gap_start as u64;
            let to = gap.gap_end as u64;
            let jobs_in_range = self.jobs_from_block_range(from, to, Some(1)).await?;
            if !jobs_in_range.is_empty() {
                let block_number = EigenDAJob::from(jobs_in_range[0].clone()).block_number;
                // there might be multiple jobs for the same block
                for job in jobs_in_range {
                    if EigenDAJob::from(job.clone()).block_number == block_number {
                        jobs.push(job);
                    } else {
                        break;
                    }
                }
                if block_number < to {
                    new_gaps.push(Gap {
                        gap_start: block_number as i64 + 1,
                        gap_end: to as i64,
                    });
                }
            }
        }
        *unprocessed_gaps = new_gaps;
        Ok(jobs)
    }
}
