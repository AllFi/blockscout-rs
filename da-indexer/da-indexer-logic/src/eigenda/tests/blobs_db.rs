use da_indexer_entity::{
    eigenda_batches,
    eigenda_blobs::{Column, Entity},
};
use sea_orm::{
    DbBackend, EntityTrait, JoinType, QueryOrder, QuerySelect, QueryTrait, SelectColumns,
};

#[tokio::test]
async fn find_blob_test() {
    println!(
        "{:?}",
        Entity::find_by_id(vec![1, 2, 3])
            .join_rev(
                JoinType::LeftJoin,
                eigenda_batches::Entity::belongs_to(Entity)
                    .from(eigenda_batches::Column::BatchHeaderHash)
                    .to(Column::BatchHeaderHash)
                    .into(),
            )
            .order_by_desc(eigenda_batches::Column::BatchId)
            .select_column(eigenda_batches::Column::BatchId)
            .select_column(eigenda_batches::Column::L1TxHash)
            .select_column(eigenda_batches::Column::L1Block)
            .limit(1)
            .build(DbBackend::Postgres)
            .to_string()
    )
}
