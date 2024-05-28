//pub mod client;
//mod indexer;
pub mod batches_db;
pub mod blobs_db;

use blockscout_service_launcher::test_database::TestDbGuard;

pub async fn init_db(test_name: &str) -> TestDbGuard {
    TestDbGuard::new::<migration::Migrator>(test_name).await
}
