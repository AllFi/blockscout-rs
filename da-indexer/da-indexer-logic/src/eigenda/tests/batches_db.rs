use sea_orm::DatabaseConnection;

use crate::eigenda::{repository::batches, tests::init_db};

#[tokio::test]
async fn find_gaps() {
    let db = init_db("batches_db_find_gaps").await;

    let heights = vec![7, 12, 13, 14, 15, 17, 94, 156, 157];
    insert_batches(&db.client(), heights).await;

    let gaps = batches::find_gaps(&db.client(), 53, 20009).await.unwrap();
    assert!(gaps[0].gap_start == 53 && gaps[0].gap_end == 699);
    assert!(gaps[1].gap_start == 701 && gaps[1].gap_end == 1199);
    assert!(gaps[2].gap_start == 1501 && gaps[2].gap_end == 1699);
    assert!(gaps[3].gap_start == 1701 && gaps[3].gap_end == 9399);
    assert!(gaps[4].gap_start == 9401 && gaps[4].gap_end == 15599);
    assert!(gaps[5].gap_start == 15701 && gaps[5].gap_end == 20009);
}

#[tokio::test]
async fn find_min_batch_id() {
    let db = init_db("batches_db_find_min_batch_id").await;

    let (batch_id, l1_block) = batches::find_min_batch_id(&db.client())
        .await
        .unwrap()
        .unwrap_or((0, 0));
}

async fn insert_batches(db: &DatabaseConnection, batches: Vec<i64>) {
    // for simplicity l1_blocks = batch_id * 100
    let l1_tx_hash = vec![5, 6, 7, 8];
    for batch_id in batches.iter() {
        batches::upsert(
            db,
            &vec![(*batch_id % 256) as u8, 0, 0][..],
            *batch_id,
            10,
            &l1_tx_hash,
            *batch_id * 100,
        )
        .await
        .unwrap();
    }
}
