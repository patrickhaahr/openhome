use std::io::Read;

use flate2::read::MultiGzDecoder;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use super::super::replace_window::{self, PartOutcome};

use super::parse::parse_batch;
use super::{Batch, BatchDelivery, BatchHistory, IngestError, MAX_DECODED_BYTES, upsert};

/// Content coding of the request entity. Batch identity is the decoded bytes, never the coding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentCoding {
    Identity,
    Gzip,
}

impl ContentCoding {
    /// Parses a single `Content-Encoding` value (absent means identity). Returns `None` for any
    /// other coding, including stacked codings.
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

/// Removes the content coding, enforcing the decoded bound while decompressing so a small gzip
/// entity cannot expand without limit.
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

/// Result of a successfully accepted (or replayed) batch.
#[derive(Debug)]
pub struct Accepted {
    /// The acknowledgement JSON, byte-identical on every replay.
    pub ack: String,
    pub stream: &'static str,
    pub records: usize,
    /// True when the ack came from the ledger and no rows were written.
    pub replayed: bool,
}

/// A decoded, validated batch, its decoded entity (staged for replace-window parts) and the
/// entity's SHA-256, ready to apply.
#[derive(Debug)]
pub struct PreparedBatch {
    batch: Batch,
    entity: Vec<u8>,
    body_sha256: Vec<u8>,
}

/// The CPU-bound half of ingest (decode up to 4 MiB, frame, validate, hash). Synchronous so the
/// caller can run it off the async workers.
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

/// Applies a prepared batch idempotently. The ledger decides first ([`BatchHistory`]): a `batchId`
/// with different bytes is a conflict; an append batch replays the ledger ack for a byte-identical
/// retry or upserts its records; a replace-window part is staged and the replacement applied once
/// complete. Rows and the ledger acknowledgement commit together.
pub async fn apply(pool: &SqlitePool, prepared: PreparedBatch) -> Result<Accepted, IngestError> {
    let PreparedBatch {
        batch,
        entity,
        body_sha256,
    } = prepared;

    // IMMEDIATE takes the write lock up front, so concurrent retries of one batch serialize on
    // the ledger instead of both applying it.
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    // Rotation clears the ledger and replacement metadata in the same transaction as the ID
    // update (receiver_state_rotation trigger), starting a fresh idempotency generation.
    let receiver_state_id: String =
        sqlx::query_scalar("SELECT receiver_state_id FROM receiver_state WHERE id = 1")
            .fetch_one(&mut *tx)
            .await?;

    // A conflicting `batchId` is rejected here; dropping the transaction rolls it back and
    // nothing was written.
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
                // `accept` only reports a retry for a batch the ledger already holds.
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
