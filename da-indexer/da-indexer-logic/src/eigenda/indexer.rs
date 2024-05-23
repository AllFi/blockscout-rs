use std::{
    cmp::{max, min},
    sync::atomic::AtomicU64,
    time::Duration,
};

use super::{client::Client, common_transport::CommonTransport, settings::IndexerSettings};
use anyhow::{bail, Error, Result};
use ethers::{
    providers::{Middleware, Provider},
    types::{Address, Filter, Log, Topic},
};
use futures::{
    stream::{self, repeat_with, BoxStream},
    Stream, StreamExt,
};
use tokio::time::sleep;
use tracing::instrument;

pub struct Job {
    batch_header_hash: Vec<u8>,
    tx_hash: ethers::types::H256,
}

impl TryFrom<Log> for Job {
    type Error = anyhow::Error;

    fn try_from(log: Log) -> Result<Self, Self::Error> {
        if log.removed == Some(true) {
            bail!("unexpected pending log")
        }
        let batch_header_hash = log.topics.get(1).unwrap().as_bytes().to_vec();
        let tx_hash = log
            .transaction_hash
            .ok_or(anyhow::anyhow!("unexpected pending log"))?;
        Ok(Self {
            tx_hash,
            batch_header_hash,
        })
    }
}

pub struct Indexer {
    settings: IndexerSettings,

    client: Client,
    provider: Provider<CommonTransport>,
    last_known_block: AtomicU64,
}

// TODO: this implementation will probably look veery similar to the one in Celestia
// Perhaps, it makes sense to implement some generic indexer implementation
// and move out da specific parts
impl Indexer {
    pub async fn new(settings: IndexerSettings) -> Result<Self> {
        let transport = CommonTransport::new(settings.rpc.url.clone()).await?;
        let provider = Provider::new(transport);
        // TODO: add retry delays to settings
        let client = Client::new(settings.disperser.clone(), vec![5, 20]);
        Ok(Self {
            settings,
            client,
            provider,
            last_known_block: AtomicU64::new(1589491),
        })
    }

    #[instrument(name = "eigenda_indexer", skip_all)]
    pub async fn start(&self) -> anyhow::Result<()> {
        let mut stream = stream::SelectAll::<BoxStream<Job>>::new();
        //stream.push(Box::pin(self.catch_up().await?));
        stream.push(Box::pin(self.poll_for_new_blocks()));
        //stream.push(Box::pin(self.retry_failed_blocks()));

        stream
            .for_each_concurrent(Some(self.settings.concurrency as usize), |job| async move {
                self.process_job_with_retries(&job).await
            })
            .await;

        Ok(())
    }

    async fn process_job_with_retries(&self, job: &Job) {
        let mut backoff = vec![5, 20].into_iter().map(Duration::from_secs);
        while let Err(err) = &self.process_job(job).await {
            match backoff.next() {
                None => {
                    tracing::warn!(error = ?err, tx_hash = ?job.tx_hash, "failed to process job, skipping for now, will retry later");
                    //self.failed_blocks.write().await.insert(job.height);
                    break;
                }
                Some(delay) => {
                    tracing::error!(error = ?err, tx_hash = ?job.tx_hash, ?delay, "failed to process job, retrying");
                    sleep(delay).await;
                }
            };
        }
    }

    async fn process_job(&self, job: &Job) -> Result<()> {
        tracing::info!(tx_hash = ?job.tx_hash, "processing job");
        let blobs = self
            .client
            .retrieve_blobs(job.batch_header_hash.clone())
            .await?;
        tracing::info!(count = blobs.len(), "retrieved blobs");
        Ok(())
    }

    fn poll_for_new_blocks(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(|| async {
            sleep(self.settings.polling_interval).await;

            // TODO: fix me
            let last_block = self.provider.get_block_number().await?.as_u64();
            let from = self
                .last_known_block
                .load(std::sync::atomic::Ordering::Relaxed);
            let to = min(from + self.settings.rpc.batch_size, last_block);

            let filter = Filter::new()
                .address(
                    self.settings
                        .contract_address
                        .clone()
                        .parse::<Address>()
                        .unwrap(),
                )
                .event("BatchConfirmed(bytes32,uint32)")
                .from_block(from)
                .to_block(to);
            tracing::info!(from, to, "fetching past BeforeExecution logs from rpc");

            let jobs: Vec<Job> = self
                .provider
                .get_logs(&filter)
                .await?
                .into_iter()
                .filter_map(|log| Job::try_from(log).ok())
                .collect();
            tracing::info!(count = jobs.len(), "fetched past BeforeExecution logs");
            self.last_known_block
                .store(to, std::sync::atomic::Ordering::Relaxed);
            Ok(jobs)
        })
        .filter_map(|fut| async {
            fut.await
                .map_err(
                    |err: Error| tracing::error!(error = ?err, "failed to poll for new blocks"),
                )
                .ok()
        })
        .flat_map(stream::iter)
    }
}
