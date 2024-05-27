use da_indexer_entity::eigenda_blobs::{ActiveModel, Column, Entity, Model};
use sea_orm::{sea_query::OnConflict, ConnectionTrait, EntityTrait};
use sha3::{Digest, Sha3_256};

pub async fn upsert_many<C: ConnectionTrait>(
    db: &C,
    batch_header_hash: &[u8],
    blobs: Vec<Vec<u8>>,
) -> Result<(), anyhow::Error> {
    let blobs = blobs.into_iter().enumerate().map(|(blob_index, data)| {
        let model = Model {
            id: compute_id(batch_header_hash, blob_index as i32),
            batch_header_hash: batch_header_hash.to_vec(),
            blob_index: blob_index as i32,
            data,
        };
        let active: ActiveModel = model.into();
        active
    });

    Entity::insert_many(blobs)
        .on_conflict(OnConflict::column(Column::Id).do_nothing().to_owned())
        .on_empty_do_nothing()
        .exec(db)
        .await?;
    Ok(())
}

fn compute_id(batch_header_hash: &[u8], blob_index: i32) -> Vec<u8> {
    Sha3_256::digest([batch_header_hash, &blob_index.to_be_bytes()[..]].concat())
        .as_slice()
        .to_vec()
}
