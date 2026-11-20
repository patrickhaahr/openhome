use std::io::Read;

use flate2::read::MultiGzDecoder;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use super::super::replace_window::{self, PartOutcome};

use super::parse::parse_batch;
use super::{Batch, BatchDelivery, BatchHistory, IngestError, MAX_DECODED_BYTES, upsert};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentCoding {
    Identity,
    Gzip,
}

impl ContentCoding {
    #[must_use]
    pub fn parse(value: Option<&str>) -> Option<Self> {
        let value = value.map(|value| value.trim().to_ascii_lowercase());
        match value.as_deref() {
            None | Some("identity") => Some(Self::Identity),
            Some("gzip" | "x-gzip") => Some(Self::Gzip),
            _ => None,
        }
    }
}

pub fn decode_entity(coding: ContentCoding, wire: Vec<u8>) -> Result<Vec<u8>, IngestError> {
    let decoded = match coding {
        ContentCoding::Identity => wire,
        ContentCoding::Gzip => {
            let mut decoded = Vec::new();
            MultiGzDecoder::new(wire.as_slice())
                .take(MAX_DECODED_BYTES as u64 + 1)
                .read_to_end(&mut decoded)
                .map_err(|_| IngestError::Malformed("invalid_content_encoding"))?;
            decoded
        }
    };
    if decoded.len() > MAX_DECODED_BYTES {
        return Err(IngestError::TooLarge);
    }
    Ok(decoded)
}

#[derive(Debug)]
pub struct Accepted {
    pub ack: String,
    pub stream: &'static str,
    pub records: usize,
    pub replayed: bool,
}

#[derive(Debug)]
pub struct PreparedBatch {
    batch: Batch,
    entity: Vec<u8>,
    body_sha256: Vec<u8>,
}

pub fn prepare(coding: ContentCoding, wire: Vec<u8>) -> Result<PreparedBatch, IngestError> {
    let entity = decode_entity(coding, wire)?;
    let batch = parse_batch(&entity)?;
    let body_sha256 = Sha256::digest(&entity).to_vec();
    Ok(PreparedBatch {
        batch,
        entity,
        body_sha256,
    })
}

pub async fn apply(pool: &SqlitePool, prepared: PreparedBatch) -> Result<Accepted, IngestError> {
    let PreparedBatch {
        batch,
        entity,
        body_sha256,
    } = prepared;

    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let receiver_state_id: String =
        sqlx::query_scalar("SELECT receiver_state_id FROM receiver_state WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;

    let history = BatchHistory::read(&mut tx, &receiver_state_id, &batch, &body_sha256).await?;
    let replay = |ack: String| Accepted {
        ack,
        stream: batch.stream.name,
        records: batch.records.len(),
        replayed: true,
    };

    match &batch.delivery {
        BatchDelivery::Append { .. } => match &history {
            BatchHistory::Identical { ack } => return Ok(replay(ack.clone())),
            BatchHistory::Unseen => {
                upsert(
                    &mut tx,
                    batch.stream,
                    &batch.source_id,
                    &batch.device_id,
                    &batch.records,
                )
                .await?;
            }
        },
        BatchDelivery::ReplaceWindow(window) => {
            let outcome = replace_window::accept(
                &mut tx,
                &receiver_state_id,
                &batch,
                window,
                &entity,
                &history,
            )
            .await?;
            match (outcome, &history) {
                (PartOutcome::Retry, BatchHistory::Identical { ack }) => {
                    return Ok(replay(ack.clone()));
                }
                (PartOutcome::Retry, BatchHistory::Unseen) => {
                    return Err(IngestError::Internal("retry_without_ledger_entry"));
                }
                (PartOutcome::Staged | PartOutcome::Applied, _) => {}
            }
        }
    }

    let ack = batch.ack();
    sqlx::query(
        "INSERT INTO batch_ledger \
             (receiver_state_id, source_id, device_id, batch_id, stream, body_sha256, ack) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&receiver_state_id)
    .bind(&batch.source_id)
    .bind(&batch.device_id)
    .bind(&batch.batch_id)
    .bind(batch.stream.name)
    .bind(body_sha256.as_slice())
    .bind(&ack)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Accepted {
        ack,
        stream: batch.stream.name,
        records: batch.records.len(),
        replayed: false,
    })
}
