use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE "eigenda_batches" (
                "batch_header_hash" bytea PRIMARY KEY,
                "batch_id" bigint NOT NULL,
                "blobs_count" integer NOT NULL,
                "timestamp" bigint NOT NULL,
                "l1_tx_hash" bytea NOT NULL,
                "indexing_status" integer NOT NULL
            );
            
            CREATE TABLE "eigenda_blobs" (
                "id" bytea PRIMARY KEY,
                "batch_header_hash" bytea NOT NULL references "eigenda_batches"("batch_header_hash"),
                "blob_index" integer NOT NULL,
                "data" bytea NOT NULL
            );

            COMMENT ON TABLE "eigenda_batches" IS 'Table contains eigenda batches metadata';

            COMMENT ON TABLE "eigenda_blobs" IS 'Table contains eigenda blobs';
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "eigenda_batches";
            DROP TABLE "eigenda_blobs";
        "#;

        crate::from_sql(manager, sql).await
    }
}
