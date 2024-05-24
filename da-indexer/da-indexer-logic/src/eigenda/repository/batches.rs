use da_indexer_entity::eigenda_batches::{ActiveModel, Column, Entity, Model};
use sea_orm::{sea_query::{Expr, OnConflict}, ConnectionTrait, ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, QueryFilter, QuerySelect, Statement};

#[derive(FromQueryResult, Debug, Clone)]
pub struct Gap {
    pub gap_start: i64,
    pub gap_end: i64,
}

pub async fn find_gaps(
    db: &DatabaseConnection,
    contract_creation_block: i64,
    to_block: i64,
) -> Result<Vec<Gap>, anyhow::Error> {
    let mut gaps = Gap::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT l1_block + 1 as gap_start, 
                next_l1_block - 1 as gap_end
        FROM (
            SELECT batch_id, l1_block, lead(batch_id) OVER (ORDER BY batch_id) as next_batch_id, 
                lead(l1_block) OVER (ORDER BY batch_id) as next_l1_block
            FROM eigenda_batches WHERE l1_block <= 1601513
        ) nr
        WHERE nr.batch_id + 1 <> nr.next_batch_id ORDER BY nr.batch_id;"#,
        [to_block.into()],
    ))
    .all(db)
    .await?;
    println!("1: {:?}", gaps);

    // adding the gap between the contract creation block and the first saved batch
    let (min_batch_id, min_l1_block) = find_min_batch_id(db).await?.unwrap_or((1, to_block + 1));
    if min_batch_id > 0 {
        gaps = [&[Gap {
            gap_start: contract_creation_block,
            gap_end: min_l1_block - 1,
        }], &gaps[..]].concat();
    } 
    println!("2: {:?}", gaps);

    // adding the gap between the last saved batch and the to_block
    let gaps_end = gaps.last().map(|gap| gap.gap_end).unwrap_or(0);
    let max_height = find_max_l1_block_in_range(db, gaps_end, to_block)
        .await?
        .unwrap_or(0);
    if max_height < to_block {
        gaps.push(Gap {
            gap_start: (max_height + 1) as i64,
            gap_end: to_block as i64,
        })
    }
    println!("3: {:?}", gaps);

    Ok(gaps)
}

pub async fn find_max_l1_block_in_range(
    db: &DatabaseConnection,
    from: i64,
    to: i64,
) -> Result<Option<i64>, anyhow::Error> {
    let max_block = Entity::find()
        .select_only()
        .column_as(Expr::col(Column::L1Block).max(), "l1_block")
        .filter(Column::L1Block.gte(from))
        .filter(Column::L1Block.lte(to))
        .into_tuple()
        .one(db)
        .await?;
    Ok(max_block)
}

pub async fn find_min_batch_id(
    db: &DatabaseConnection,
) -> Result<Option<(i64, i64)>, anyhow::Error> {
    let min_block = Entity::find()
        .select_only()
        .column_as(Expr::col(Column::BatchId).min(), "batch_id")
        .column_as(Expr::col(Column::L1Block).min(), "l1_block")
        .into_tuple()
        .one(db)
        .await?;
    Ok(min_block)
}

pub async fn upsert<C: ConnectionTrait>(
    db: &C,
    batch_header_hash: &[u8],
    batch_id: i64,
    blobs_count: i32,
    l1_tx_hash: &[u8],
    l1_block: i64,
) -> Result<(), anyhow::Error> {
    let model = Model {
        batch_header_hash: batch_header_hash.to_vec(),
        batch_id,
        blobs_count,
        l1_tx_hash: l1_tx_hash.to_vec(),
        l1_block,
    };
    let active: ActiveModel = model.into();

    Entity::insert(active)
        .on_conflict(
            OnConflict::column(Column::BatchHeaderHash)
                .update_columns([Column::BatchId, Column::BlobsCount, Column::L1TxHash, Column::L1Block])
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}