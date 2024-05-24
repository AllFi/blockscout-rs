use anyhow::{Error, Result};
use async_trait::async_trait;
use futures::{
    stream,
    stream::{repeat_with, BoxStream},
    Stream, StreamExt,
};
use sea_orm::DatabaseConnection;
use std::{collections::HashSet, sync::Arc, time::Duration};
use std::{fmt::Debug, hash::Hash};
use tokio::{sync::RwLock, time::sleep};
use tracing::instrument;

use crate::{
    celestia, eigenda,
    settings::{DASettings, IndexerSettings},
};

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub enum Job {
    Celestia(celestia::da::CelestiaJob),
    EigenDA(eigenda::da::EigenDAJob),
}

#[async_trait]
pub trait DA {
    async fn process_job(&self, job: Job) -> Result<()>;
    async fn unprocessed_jobs(&self) -> Result<Vec<Job>>;
    async fn new_jobs(&self) -> Result<Vec<Job>>;
}

pub struct Indexer {
    da: Box<dyn DA + Send + Sync>,
    settings: IndexerSettings,

    failed_jobs: RwLock<HashSet<Job>>,
}

impl Indexer {
    pub async fn new(db: Arc<DatabaseConnection>, settings: IndexerSettings) -> Result<Self> {
        let da: Box<dyn DA + Send + Sync> = match settings.da.clone() {
            DASettings::Celestia(settings) => {
                Box::new(celestia::da::CelestiaDA::new(db.clone(), settings).await?)
            }
            DASettings::EigenDA(settings) => {
                Box::new(eigenda::da::EigenDA::new(db.clone(), settings).await?)
            }
        };
        Ok(Self {
            da,
            settings,
            failed_jobs: RwLock::new(HashSet::new()),
        })
    }

    //#[instrument(name = I::name(), skip_all)]
    pub async fn start(&self) -> anyhow::Result<()> {
        let mut stream = stream::SelectAll::<BoxStream<Job>>::new();
        stream.push(Box::pin(self.catch_up().await?));
        stream.push(Box::pin(self.poll_for_new_blocks()));
        stream.push(Box::pin(self.retry_failed_blocks()));

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
                    tracing::warn!(error = ?err, job = ?job, "failed to process job, skipping for now, will retry later");
                    self.failed_jobs.write().await.insert(job.clone());
                    break;
                }
                Some(delay) => {
                    tracing::error!(error = ?err, job = ?job, ?delay, "failed to process job, retrying");
                    sleep(delay).await;
                }
            };
        }
    }

    async fn process_job(&self, job: &Job) -> Result<()> {
        self.da.process_job(job.clone()).await
    }

    async fn catch_up(&self) -> Result<impl Stream<Item = Job> + '_> {
        self.da.unprocessed_jobs().await.map(stream::iter)
    }

    fn poll_for_new_blocks(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(|| async {
            sleep(self.settings.polling_interval).await;
            self.da.new_jobs().await
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

    fn retry_failed_blocks(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(|| async {
            sleep(self.settings.retry_interval).await;
            // we can safely drain the failed blocks here
            // if the block fails again, it will be re-added
            // if the indexer will be restarted, catch_up will take care of it
            let iter = self.failed_jobs.write().await.drain().collect::<Vec<_>>();
            tracing::info!("retrying failed jobs: {:?}", iter);
            Ok(iter)
        })
        .filter_map(|fut| async {
            fut.await
                .map_err(|err: Error| tracing::error!(error = ?err, "failed to retry failed jobs"))
                .ok()
        })
        .flat_map(stream::iter)
    }
}
