use blockscout_service_launcher::tracing::{JaegerSettings, TracingSettings};

use crate::eigenda::{indexer::Indexer, settings::IndexerSettings};

#[tokio::test]
pub async fn indexer_test() {
    blockscout_service_launcher::tracing::init_logs(
        "eigenda_indexer",
        &TracingSettings::default(),
        &JaegerSettings::default(),
    ).unwrap();
    let indexer = Indexer::new(IndexerSettings::default()).await.unwrap();
    indexer.start().await.unwrap();
}
