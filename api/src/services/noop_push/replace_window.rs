//! Replace-window delivery for the mutable streams (`.noop/PUSH_PROTOCOL.md`, "Authoritative
//! rolling-window delivery").
//!
//! Every part belongs to one replacement (`replacementId`) whose stream, window and part count
//! are fixed by the first part observed. Parts are staged durably (`replacement_part`); the part
//! that completes the set triggers, in the same transaction, the atomic apply: upsert every
//! record by scoped primary key, then delete the rows in `(source, device)` and the window whose
//! keys are absent from the complete replacement. Its ack is only returned once that commits.
//!
//! Generations: `replacement_scope` remembers the latest replacement observed per
//! `(source, device, stream)`. A part of any other replacement starts a new generation and
//! supersedes the current one if it is still staging; superseded replacements are fences whose
//! parts (late or retried) are rejected with 409.
//!
//! Retries: a byte-identical accepted part replays its ack without touching rows or changing
//! the current generation, unless its incomplete replacement was superseded. This also keeps
//! late retries of completed replacements from overwriting newer data.

use std::collections::HashSet;

use sqlx::{Row, SqliteConnection};

use super::ingest::{Batch, DUPLICATE_KEY, IngestError, KeyValue, Window, parse_batch, upsert};
use super::registry::{ColumnType, ReplaceWindow};

const REPLACEMENT_CONFLICT: IngestError = IngestError::Conflict("replacement_conflict");
const REPLACEMENT_SUPERSEDED: IngestError = IngestError::Conflict("replacement_superseded");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PartOutcome {
    /// Byte-identical retry of an accepted part; nothing changed.
    Retry,
    /// Staged; the replacement is still waiting for other parts.
    Staged,
    /// This part completed the replacement and it was applied.
    Applied,
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    Staging,
    Applied,
}

/// Row identity shared by every replacement table.
struct Scope<'a> {
    receiver_state_id: &'a str,
    source_id: &'a str,
    device_id: &'a str,
}

/// Accepts one validated replace-window part inside the caller's write transaction.
/// `known_batch` is true when the ledger already holds this `batchId` with identical bytes.
pub(super) async fn accept(
    conn: &mut SqliteConnection,
    receiver_state_id: &str,
    batch: &Batch,
    window: &Window,
    entity: &[u8],
    known_batch: bool,
) -> Result<PartOutcome, IngestError> {
    let scope = Scope {
        receiver_state_id,
        source_id: &batch.source_id,
        device_id: &batch.device_id,
    };
    let spec = batch
        .stream
        .replace_window()
        .ok_or(IngestError::Internal("not_a_replace_window_stream"))?;
    let (start, end) = window.bounds.canonical();
    let selector = window.bounds.selector().wire_name();
    let replacement_id = window.replacement_id.as_str();
    let part = i64::from(window.part);
    let parts = i64::from(window.parts);

    let existing: Option<(String, String, String, String, i64, String)> = sqlx::query_as(
        "SELECT stream, selector, start_inclusive, end_exclusive, parts, state FROM replacement \
         WHERE receiver_state_id = ? AND source_id = ? AND device_id = ? AND replacement_id = ?",
    )
    .bind(scope.receiver_state_id)
    .bind(scope.source_id)
    .bind(scope.device_id)
    .bind(replacement_id)
    .fetch_optional(&mut *conn)
    .await?;
    let state = match existing {
        None => None,
        Some((stored_stream, stored_selector, stored_start, stored_end, stored_parts, state)) => {
            let same = stored_stream == batch.stream.name
                && stored_selector == selector
                && stored_start == start
                && stored_end == end
                && stored_parts == parts;
            if !same {
                return Err(REPLACEMENT_CONFLICT);
            }
            match state.as_str() {
                "staging" => Some(State::Staging),
                "applied" => Some(State::Applied),
                _ => return Err(REPLACEMENT_SUPERSEDED),
            }
        }
    };

    let stored_part: Option<String> = sqlx::query_scalar(
        "SELECT batch_id FROM replacement_part \
         WHERE receiver_state_id = ? AND source_id = ? AND device_id = ? \
         AND replacement_id = ? AND part = ?",
    )
    .bind(scope.receiver_state_id)
    .bind(scope.source_id)
    .bind(scope.device_id)
    .bind(replacement_id)
    .bind(part)
    .fetch_optional(&mut *conn)
    .await?;
    if stored_part.is_some_and(|stored_batch_id| stored_batch_id != batch.batch_id) {
        return Err(REPLACEMENT_CONFLICT);
    }
    // Superseded replacements were rejected above, even for a previously accepted part.
    // Every other known batch replays before it can change the scope's current generation.
    if known_batch {
        return Ok(PartOutcome::Retry);
    }
    if state == Some(State::Applied) {
        return Err(REPLACEMENT_CONFLICT);
    }

    let current: Option<String> = sqlx::query_scalar(
        "SELECT current_replacement_id FROM replacement_scope \
         WHERE receiver_state_id = ? AND source_id = ? AND device_id = ? AND stream = ?",
    )
    .bind(scope.receiver_state_id)
    .bind(scope.source_id)
    .bind(scope.device_id)
    .bind(batch.stream.name)
    .fetch_optional(&mut *conn)
    .await?;

    if current.as_deref() != Some(replacement_id) {
        begin_generation(
            conn,
            &scope,
            batch,
            window,
            current.as_deref(),
            (&start, &end),
        )
        .await?;
    }

    sqlx::query(
        "INSERT INTO replacement_part \
         (receiver_state_id, source_id, device_id, replacement_id, part, batch_id, entity) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(scope.receiver_state_id)
    .bind(scope.source_id)
    .bind(scope.device_id)
    .bind(replacement_id)
    .bind(part)
    .bind(&batch.batch_id)
    .bind(entity)
    .execute(&mut *conn)
    .await?;

    let staged: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM replacement_part \
         WHERE receiver_state_id = ? AND source_id = ? AND device_id = ? \
         AND replacement_id = ? AND entity IS NOT NULL",
    )
    .bind(scope.receiver_state_id)
    .bind(scope.source_id)
    .bind(scope.device_id)
    .bind(replacement_id)
    .fetch_one(&mut *conn)
    .await?;
    if staged < parts {
        return Ok(PartOutcome::Staged);
    }

    apply_replacement(conn, &scope, batch, window, spec).await?;
    Ok(PartOutcome::Applied)
}

/// Makes `window.replacement_id` the scope's current generation, superseding the previous one
/// if it is still staging, and opens the new replacement for staging.
async fn begin_generation(
    conn: &mut SqliteConnection,
    scope: &Scope<'_>,
    batch: &Batch,
    window: &Window,
    previous: Option<&str>,
    (start, end): (&str, &str),
) -> Result<(), sqlx::Error> {
    if let Some(previous) = previous {
        sqlx::query(
            "UPDATE replacement SET state = 'superseded' \
             WHERE receiver_state_id = ? AND source_id = ? AND device_id = ? \
             AND replacement_id = ? AND state = 'staging'",
        )
        .bind(scope.receiver_state_id)
        .bind(scope.source_id)
        .bind(scope.device_id)
        .bind(previous)
        .execute(&mut *conn)
        .await?;
        clear_staged(conn, scope, previous).await?;
    }
    sqlx::query(
        "INSERT INTO replacement_scope \
         (receiver_state_id, source_id, device_id, stream, current_replacement_id) \
         VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT (receiver_state_id, source_id, device_id, stream) \
         DO UPDATE SET current_replacement_id = excluded.current_replacement_id",
    )
    .bind(scope.receiver_state_id)
    .bind(scope.source_id)
    .bind(scope.device_id)
    .bind(batch.stream.name)
    .bind(&window.replacement_id)
    .execute(&mut *conn)
    .await?;
    sqlx::query(
        "INSERT INTO replacement (receiver_state_id, source_id, device_id, replacement_id, \
         stream, selector, start_inclusive, end_exclusive, parts, state) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'staging')",
    )
    .bind(scope.receiver_state_id)
    .bind(scope.source_id)
    .bind(scope.device_id)
    .bind(&window.replacement_id)
    .bind(batch.stream.name)
    .bind(window.bounds.selector().wire_name())
    .bind(start)
    .bind(end)
    .bind(i64::from(window.parts))
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// The atomic apply of a complete replacement: upsert every part's records, delete absent keys
/// in the window, mark the replacement applied and drop its staged entities.
async fn apply_replacement(
    conn: &mut SqliteConnection,
    scope: &Scope<'_>,
    batch: &Batch,
    window: &Window,
    spec: &ReplaceWindow,
) -> Result<(), IngestError> {
    let stream = batch.stream;
    let mut keys = HashSet::new();
    for part in 1..=window.parts {
        let staged;
        let records = if part == window.part {
            &batch.records
        } else {
            let entity: Vec<u8> = sqlx::query_scalar(
                "SELECT entity FROM replacement_part \
                 WHERE receiver_state_id = ? AND source_id = ? AND device_id = ? \
                 AND replacement_id = ? AND part = ?",
            )
            .bind(scope.receiver_state_id)
            .bind(scope.source_id)
            .bind(scope.device_id)
            .bind(&window.replacement_id)
            .bind(i64::from(part))
            .fetch_one(&mut *conn)
            .await?;
            staged =
                parse_batch(&entity).map_err(|_| IngestError::Internal("corrupt_staged_part"))?;
            &staged.records
        };
        // Keys are unique within a part (checked at parse); the complete window must be too.
        for record in records {
            if !keys.insert(record.key.clone()) {
                return Err(DUPLICATE_KEY);
            }
        }
        upsert(conn, stream, scope.source_id, scope.device_id, records).await?;
    }

    let query = sqlx::query(spec.window_keys_sql)
        .bind(scope.source_id)
        .bind(scope.device_id);
    let stored = window.bounds.bind(query).fetch_all(&mut *conn).await?;
    let mut deleted = 0_u64;
    for row in stored {
        let key = stream
            .key
            .iter()
            .enumerate()
            .map(|(index, column)| match column.ty {
                ColumnType::Integer => row.try_get(index).map(KeyValue::Integer),
                _ => row.try_get(index).map(KeyValue::Text),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if keys.contains(&key) {
            continue;
        }
        let mut delete = sqlx::query(spec.delete_sql)
            .bind(scope.source_id)
            .bind(scope.device_id);
        for value in &key {
            delete = value.bind(delete);
        }
        deleted += delete.execute(&mut *conn).await?.rows_affected();
    }

    sqlx::query(
        "UPDATE replacement SET state = 'applied' \
         WHERE receiver_state_id = ? AND source_id = ? AND device_id = ? AND replacement_id = ?",
    )
    .bind(scope.receiver_state_id)
    .bind(scope.source_id)
    .bind(scope.device_id)
    .bind(&window.replacement_id)
    .execute(&mut *conn)
    .await?;
    clear_staged(conn, scope, &window.replacement_id).await?;

    tracing::info!(
        stream = stream.name,
        parts = window.parts,
        upserted = keys.len(),
        deleted,
        "NOOP push replacement applied"
    );
    Ok(())
}

async fn clear_staged(
    conn: &mut SqliteConnection,
    scope: &Scope<'_>,
    replacement_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE replacement_part SET entity = NULL \
         WHERE receiver_state_id = ? AND source_id = ? AND device_id = ? AND replacement_id = ?",
    )
    .bind(scope.receiver_state_id)
    .bind(scope.source_id)
    .bind(scope.device_id)
    .bind(replacement_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}
