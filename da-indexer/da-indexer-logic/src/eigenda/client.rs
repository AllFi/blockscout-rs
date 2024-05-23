use std::time::Duration;

use self::disperser::{
    disperser_client::DisperserClient, disperser_server::Disperser, RetrieveBlobRequest,
};
use anyhow::Result;
use tokio::time::sleep;
use tonic::{transport::Channel, Status};

mod disperser {
    tonic::include_proto!("disperser");
}

mod common {
    tonic::include_proto!("common");
}

pub struct Client {
    disperser_endpoint: String,
    retry_delays: Vec<u64>,
}

impl Client {
    pub fn new(disperser_endpoint: String, retry_delays: Vec<u64>) -> Self {
        Self {
            disperser_endpoint,
            retry_delays,
        }
    }

    pub async fn retrieve_blobs(&self, batch_header_hash: Vec<u8>) -> Result<Vec<Vec<u8>>> {
        //? can we figure out the blobs count beforehand?
        let mut blobs = vec![];
        let mut blob_index = 0;
        //? should we cache it?
        let mut client = DisperserClient::connect(self.disperser_endpoint.clone()).await?;
        while let Some(blob) = self
            .retrieve_blob_with_retries(&mut client, batch_header_hash.clone(), blob_index)
            .await?
        {
            blobs.push(blob);
            blob_index += 1;
        }
        Ok(blobs)
    }

    async fn retrieve_blob_with_retries(
        &self,
        client: &mut DisperserClient<Channel>,
        batch_header_hash: Vec<u8>,
        blob_index: u32,
    ) -> Result<Option<Vec<u8>>> {
        let mut backoff = self
            .retry_delays
            .clone()
            .into_iter()
            .map(Duration::from_secs);
        let mut last_err = Status::new(tonic::Code::Unknown, "Unknown error");
        for delay in &mut backoff {
            match Self::retrieve_blob(client, batch_header_hash.clone(), blob_index).await {
                Ok(blob) => return Ok(Some(blob)),
                Err(e) => {
                    if e.code() == tonic::Code::NotFound {
                        return Ok(None);
                    }
                    if e.code() != tonic::Code::ResourceExhausted {
                        tracing::warn!(
                            batch_header_hash = hex::encode(batch_header_hash.clone()),
                            blob_index,
                            ?delay,
                            "Failed to retrieve blob: {}, retrying",
                            e
                        );
                    }
                    last_err = e;
                    sleep(delay).await;
                }
            }
        }
        tracing::error!(
            batch_header_hash = hex::encode(batch_header_hash.clone()),
            blob_index,
            "Failed to retrieve blob: {}, skipping the batch",
            last_err
        );
        Err(last_err.into())
    }

    async fn retrieve_blob(
        client: &mut DisperserClient<Channel>,
        batch_header_hash: Vec<u8>,
        blob_index: u32,
    ) -> Result<Vec<u8>, Status> {
        let retrieve_request = tonic::Request::new(RetrieveBlobRequest {
            batch_header_hash: batch_header_hash.clone(),
            blob_index: blob_index as u32,
        });
        client
            .retrieve_blob(retrieve_request)
            .await
            .map(|response| response.into_inner().data)
    }
}
