use axum::{Router, routing::get};

use crate::error::{AppError, Result};

mod body_weight;
mod exercises;
mod profile;
mod workout_detail;
mod workout_routes;

pub use body_weight::{BodyWeightEntry, CreateBodyWeight, ListBodyWeight};
use body_weight::{create_body_weight, list_body_weight};
pub use exercises::{
    CombinedProgressQuery, CreateExercise, Exercise, ExerciseProgress, ListExercises,
    ProgressQuery, UpdateExercise,
};
use exercises::{
    combined_progress, create_exercise, delete_exercise, exercise_progress, get_exercise,
    list_exercises, update_exercise,
};
pub use profile::{Profile, UpdateProfile};
use profile::{get_profile, update_profile};
pub use workout_detail::WorkoutDetail;
pub use workout_routes::workout_router;
pub use workout_routes::{CreateWorkout, ListWorkouts, UpdateWorkout, WorkoutSummary};

/// What a stored receipt allows a retry of its `request_id`; see `docs/api-receipts.md`.
/// A retry repeating the accepted arguments is answered with the record it created (the
/// current one, not an original snapshot). Changed arguments are a conflict. Record ids can
/// be reused, so deleting the record clears the receipt's target id instead of removing the
/// receipt: a cleared target is a conflict, never a recreation. Changed arguments are
/// judged before the cleared target.
fn accepted_target(
    stored_request: &str,
    stored_target: Option<i64>,
    request: &str,
    wording: &ReplayWording,
) -> Result<i64> {
    if stored_request != request {
        return Err(AppError::Conflict(wording.changed.into()));
    }
    stored_target.ok_or_else(|| AppError::Conflict(wording.cleared.into()))
}

/// The conflict messages a domain gives to the two refusals in [`accepted_target`].
struct ReplayWording {
    changed: &'static str,
    cleared: &'static str,
}

const EXERCISE_REPLAY: ReplayWording = ReplayWording {
    changed: "request_id was already used for a different Exercise; use a new ID for a new Exercise",
    cleared: "The retried Exercise was deleted; use a new request_id to create it again",
};
const WORKOUT_REPLAY: ReplayWording = ReplayWording {
    changed: "request_id was already used for a different Workout; use a new ID for a new Workout",
    cleared: "The retried Workout was deleted; use a new request_id to log it again",
};
const BODY_WEIGHT_REPLAY: ReplayWording = ReplayWording {
    changed: "request_id was already used for a different Body Weight; use a new ID for a new record",
    cleared: "The retried Body Weight was removed; use a new request_id to record it again",
};

/// A client edit revision, which orders one client's offline edits of a Workout or the Profile.
/// An edit carrying one applies only when it is newer than the last applied (`client_revision`
/// is null until a client sends one, so the first revision always applies); an older or
/// repeated one changes nothing and is acknowledged with the current record. An edit without a
/// revision always applies and leaves the stored revision alone.
fn parse_client_revision(revision: Option<i64>) -> Result<Option<i64>> {
    if revision.is_some_and(|revision| revision < 1) {
        return Err(AppError::Validation(
            "client_revision must be 1 or greater".to_string(),
        ));
    }
    Ok(revision)
}

/// Deserializes `Option<Option<T>>` so absent and explicit `null` differ:
/// absent -> `None` (keep), `null` -> `Some(None)` (clear), value -> `Some(Some(v))`.
/// `serde_json` signals a present `null` through `visit_none` (`visit_unit` in
/// other self-describing formats); an absent field never reaches the
/// deserializer (`#[serde(default)]` supplies `None`).
mod double_option {
    use serde::{Deserialize, Deserializer, de};
    use std::marker::PhantomData;

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        D: Deserializer<'de>,
        Option<T>: Deserialize<'de>,
    {
        struct Visitor<T>(PhantomData<T>);

        impl<'de, T> de::Visitor<'de> for Visitor<T>
        where
            Option<T>: Deserialize<'de>,
        {
            type Value = Option<Option<T>>;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an optional value")
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Some(None))
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Some(None))
            }

            fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                Option::<T>::deserialize(d).map(Some)
            }
        }

        deserializer.deserialize_option(Visitor(PhantomData))
    }
}

/// Validates and normalizes to a padded `YYYY-MM-DD` string, since list
/// ordering and `?from=`/`?to=` filtering rely on lexicographic comparison.
fn validate_date(date: &str) -> Result<String> {
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation(format!("Invalid date '{date}': must be YYYY-MM-DD")))?;
    Ok(parsed.format("%Y-%m-%d").to_string())
}

pub fn router() -> Router<crate::AppState> {
    Router::new()
        .route("/api/exercises", get(list_exercises).post(create_exercise))
        .route(
            "/api/exercises/{id}",
            get(get_exercise)
                .patch(update_exercise)
                .delete(delete_exercise),
        )
        .route("/api/exercises/progress", get(combined_progress))
        .route("/api/exercises/{id}/progress", get(exercise_progress))
        .merge(workout_router())
        .merge(metrics_router())
}

pub fn metrics_router() -> Router<crate::AppState> {
    Router::new()
        .route(
            "/api/body_weight",
            get(list_body_weight).post(create_body_weight),
        )
        .route("/api/profile", get(get_profile).patch(update_profile))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct PatchSubject {
        name: Option<String>,
        #[serde(default, with = "double_option")]
        clearable: Option<Option<String>>,
    }

    #[test]
    fn double_option_distinguishes_absent_and_null() {
        // absent field: keep
        let s: PatchSubject = serde_json::from_str(r#"{"name": "x"}"#).unwrap();
        assert_eq!(s.clearable, None);
        assert_eq!(s.name, Some("x".to_string()));

        // explicit null: clear
        let s: PatchSubject = serde_json::from_str(r#"{"name": "x", "clearable": null}"#).unwrap();
        assert_eq!(s.clearable, Some(None));

        // present value: set
        let s: PatchSubject = serde_json::from_str(r#"{"name": "x", "clearable": "y"}"#).unwrap();
        assert_eq!(s.clearable, Some(Some("y".to_string())));
    }
}
