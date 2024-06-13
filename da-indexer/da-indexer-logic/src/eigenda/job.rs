use std::fmt;

use anyhow::bail;
use ethers::types::Log;

use crate::indexer::Job;

#[derive(Hash, PartialEq, Eq, Clone)]
pub struct EigenDAJob {
    pub batch_header_hash: Vec<u8>,
    pub batch_id: u64,
    pub tx_hash: ethers::types::H256,
    pub block_number: u64,
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

impl fmt::Debug for EigenDAJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Job(batchId = {})", self.batch_id)
    }
}
