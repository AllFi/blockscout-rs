use anyhow::{bail, Result};
use async_trait::async_trait;
use sea_orm::TransactionTrait;
use std::{
    cmp::min,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use ethers::{
    providers::{Middleware, Provider},
    types::{Address, Filter, Log},
};
use sea_orm::DatabaseConnection;

use crate::{
    eigenda::repository::{batches, blobs},
    indexer::{Job, DA},
};

use super::{client::Client, common_transport::CommonTransport, settings::IndexerSettings};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct EigenDAJob {
    batch_header_hash: Vec<u8>,
    batch_id: u64,
    tx_hash: ethers::types::H256,
    block_number: u64,
}

impl From<Job> for EigenDAJob {
    fn from(val: Job) -> Self {
        match val {
            Job::EigenDA(job) => job,
            _ => unreachable!(),
        }
    }
}

impl TryFrom<Log> for EigenDAJob {
    type Error = anyhow::Error;

    fn try_from(log: Log) -> Result<Self, Self::Error> {
        if log.removed == Some(true) {
            bail!("unexpected pending log")
        }
        let batch_header_hash = log.topics.get(1).unwrap().as_bytes().to_vec();
        let batch_id = u64::from_be_bytes((&log.data.to_vec()[24..32]).try_into().unwrap());
        let tx_hash = log
            .transaction_hash
            .ok_or(anyhow::anyhow!("unexpected pending log"))?;
        Ok(Self {
            batch_header_hash,
            batch_id,
            tx_hash,
            block_number: log.block_number.unwrap().as_u64(),
        })
    }
}

pub struct EigenDA {
    settings: IndexerSettings,
    db: Arc<DatabaseConnection>,

    client: Client,
    provider: Provider<CommonTransport>,
    last_known_block: AtomicU64,
}

impl EigenDA {
    pub async fn new(db: Arc<DatabaseConnection>, settings: IndexerSettings) -> Result<Self> {
        let transport = CommonTransport::new(settings.rpc.url.clone()).await?;
        let provider = Provider::new(transport);
        // TODO: add retry delays to settings
        let client = Client::new(settings.disperser.clone(), vec![5, 10, 15]);
        let start_from = settings
            .start_height
            .clone()
            .unwrap_or(provider.get_block_number().await?.as_u64());
        Ok(Self {
            settings,
            db,
            client,
            provider,
            last_known_block: AtomicU64::new(start_from.saturating_sub(1)), // TODO: check it
        })
    }

    async fn jobs_from_blocks_range(&self, from: u64, to: u64) -> Result<Vec<Job>> {
        let mut temp_from = from;
        let mut temp_to = to.min(from + self.settings.rpc.batch_size);
        let mut jobs = vec![];
        loop {
            let filter = Filter::new()
                .address(
                    self.settings
                        .contract_address
                        .clone()
                        .parse::<Address>()
                        .unwrap(),
                )
                .event("BatchConfirmed(bytes32,uint32)")
                .from_block(temp_from)
                .to_block(temp_to);
            tracing::info!(
                from = temp_from,
                to = temp_to,
                "fetching past BatchConfirmed logs from rpc"
            );

            jobs.append(
                &mut self
                    .provider
                    .get_logs(&filter)
                    .await?
                    .into_iter()
                    .filter_map(|log| EigenDAJob::try_from(log).ok().map(Job::EigenDA))
                    .collect(),
            );
            tracing::info!(count = jobs.len(), "fetched total past BatchConfirmed logs");

            temp_from = temp_to + 1;
            temp_to = to.min(temp_from + self.settings.rpc.batch_size);

            if temp_from > to {
                break;
            }
        }

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

        let txn = self.db.begin().await?;

        batches::upsert(
            &txn,
            &job.batch_header_hash,
            job.batch_id as i64,
            blobs.len() as i32,
            &job.tx_hash.as_bytes(),
            job.block_number as i64,
        )
        .await?;

        if !blobs.is_empty() {
            blobs::upsert_many(&txn, &job.batch_header_hash, blobs).await?;
        }
        txn.commit().await?;

        Ok(())
    }

    async fn new_jobs(&self) -> Result<Vec<Job>> {
        let last_block = self.provider.get_block_number().await?.as_u64();
        let from = self
            .last_known_block
            .load(std::sync::atomic::Ordering::Relaxed)
            + 1;
        let to = min(from + self.settings.rpc.batch_size, last_block);
        if to < from {
            return Ok(vec![]);
        }

        let jobs = self.jobs_from_blocks_range(from, to).await?;
        self.last_known_block
            .store(to, std::sync::atomic::Ordering::Relaxed);
        Ok(jobs)
    }

    async fn unprocessed_jobs(&self) -> Result<Vec<Job>> {
        // TODO: this function is not correct. Some batches are processed multiple times. Fix it.
        // Something definitely off here
        let gaps = batches::find_gaps(
            &self.db,
            self.settings.contract_creation_block as i64,
            self.last_known_block.load(Ordering::Relaxed) as i64,
        )
        .await?;
        tracing::info!("gaps: {:?}", gaps);

        let mut jobs = vec![];
        for gap in gaps {
            let from = gap.gap_start as u64;
            let to = gap.gap_end as u64;
            let mut jobs_in_range = self.jobs_from_blocks_range(from, to).await?;
            jobs.append(&mut jobs_in_range);
        }

        Ok(jobs)
    }
}
