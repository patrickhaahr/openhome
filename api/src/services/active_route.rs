//! Persisted, household-wide route selection. A Route Session belongs to the phone and
//! keeps its own snapshot; selecting here never starts or changes a session.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::route_loop::{Activity, GeneratedLoop, LoopPlan, LoopTarget};

/// A selection names an Axum-generated candidate; client geometry is never accepted.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub route_id: Uuid,
}

/// A measured candidate and the requested effort that produced it.
#[derive(Serialize, Deserialize)]
pub struct RouteSnapshot {
    pub activity: Activity,
    pub target: LoopTarget,
    pub step_length_cm: u32,
    pub tolerance_percent: f64,
    #[serde(flatten)]
    pub route: GeneratedLoop,
}

/// An immutable selection snapshot. Version increases only when the selected identity changes.
#[derive(Serialize)]
pub struct ActiveRoute {
    pub version: i64,
    #[serde(flatten)]
    pub snapshot: RouteSnapshot,
}

/// A selected route, or an explicit absence before the household first selects one.
#[derive(Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum ActiveRouteRead {
    None,
    Selected { route: ActiveRoute },
}

/// Selection or persistence failed without replacing the previous Active Route.
#[derive(Debug, thiserror::Error)]
pub enum ActiveRouteError {
    #[error("This loop is no longer available. Plan loops again.")]
    CandidateUnavailable,
    #[error("Route database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("Stored route snapshot could not be read")]
    Snapshot(#[from] serde_json::Error),
}

/// Remember the generated geometry before returning choices. Keep at most 96 recent
/// candidates plus the selected one, which remains available indefinitely.
pub async fn remember(db: &SqlitePool, plan: &LoopPlan) -> Result<(), ActiveRouteError> {
    let mut transaction = db.begin().await?;
    for route in &plan.loops {
        let id = route.id.to_string();
        let snapshot = serde_json::to_string(&RouteSnapshot {
            activity: plan.activity,
            target: plan.target,
            step_length_cm: plan.step_length_cm,
            tolerance_percent: plan.tolerance_percent,
            route: route.clone(),
        })?;
        // Reoffering a cached candidate refreshes retention without rewriting its snapshot.
        sqlx::query!(
            "INSERT INTO route_candidates (id, snapshot) VALUES (?, ?)
             ON CONFLICT(id) DO UPDATE SET rowid = (SELECT COALESCE(MAX(rowid), 0) + 1 FROM route_candidates)",
            id,
            snapshot
        )
        .execute(&mut *transaction)
        .await?;
    }
    sqlx::query!(
        "DELETE FROM route_candidates
         WHERE id NOT IN (SELECT route_id FROM active_route)
           AND rowid NOT IN (SELECT rowid FROM route_candidates ORDER BY rowid DESC LIMIT 96)"
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

/// Read the selected snapshot without consulting the provider or the configured origin.
pub async fn read(db: &SqlitePool) -> Result<ActiveRouteRead, ActiveRouteError> {
    let row = sqlx::query!(
        "SELECT a.version, c.snapshot FROM active_route a
         JOIN route_candidates c ON c.id = a.route_id WHERE a.slot = 1"
    )
    .fetch_optional(db)
    .await?;
    match row {
        Some(row) => Ok(ActiveRouteRead::Selected {
            route: ActiveRoute {
                version: row.version,
                snapshot: serde_json::from_str(&row.snapshot)?,
            },
        }),
        None => Ok(ActiveRouteRead::None),
    }
}

/// Atomically select only a stored candidate. Retrying the current identity is idempotent;
/// concurrent replacements serialize in SQLite and receive distinct, increasing versions.
pub async fn select(
    db: &SqlitePool,
    selection: Selection,
) -> Result<ActiveRouteRead, ActiveRouteError> {
    let id = selection.route_id.to_string();
    let mut transaction = db.begin().await?;
    let row = sqlx::query!(
        "INSERT INTO active_route (slot, route_id, version)
         SELECT 1, id, 1 FROM route_candidates WHERE id = ?
         ON CONFLICT(slot) DO UPDATE SET
           route_id = excluded.route_id,
           version = active_route.version + (active_route.route_id != excluded.route_id)
         RETURNING version",
        id
    )
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(ActiveRouteError::CandidateUnavailable)?;
    let snapshot = sqlx::query_scalar!("SELECT snapshot FROM route_candidates WHERE id = ?", id)
        .fetch_one(&mut *transaction)
        .await?;
    let route = ActiveRoute {
        version: row.version,
        snapshot: serde_json::from_str(&snapshot)?,
    };
    transaction.commit().await?;
    Ok(ActiveRouteRead::Selected { route })
}
