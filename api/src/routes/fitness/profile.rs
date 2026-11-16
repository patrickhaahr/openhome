use axum::{Json, extract::State};
use serde::Deserialize;

use super::{double_option, parse_client_revision};
use crate::error::{AppError, Result};
use crate::services::training::calendar::CalendarDay;

// ---------------------------------------------------------------------------
// Profile (#8)
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize)]
pub struct Profile {
    height_cm: Option<f64>,
    sex: Option<String>,
    hr_max_bpm: Option<i64>,
    /// `YYYY-MM-DD`; the BMR formula and the NNR age bands need it.
    birth_date: Option<String>,
    /// Where a client that downloads this Profile continues its edit revisions from.
    #[serde(skip_serializing_if = "Option::is_none")]
    client_revision: Option<i64>,
}

const HR_MAX_BPM_RANGE: std::ops::RangeInclusive<i64> = 100..=240;

#[derive(Debug, Deserialize)]
pub struct UpdateProfile {
    /// Makes this edit safe to replay: see [`update_profile`]. Without it every call applies.
    client_revision: Option<i64>,
    #[serde(default, with = "double_option")]
    height_cm: Option<Option<f64>>,
    #[serde(default, with = "double_option")]
    sex: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    hr_max_bpm: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    birth_date: Option<Option<String>>,
}

/// A birth date: a canonical `YYYY-MM-DD` day from 1900 up to today in Copenhagen.
fn parse_birth_date(text: &str) -> Result<String> {
    let invalid = || {
        AppError::Validation(
            "birth_date must be a date written YYYY-MM-DD, from 1900-01-01 up to today".to_string(),
        )
    };
    let day = CalendarDay::parse(text).ok_or_else(invalid)?;
    let first = CalendarDay::parse("1900-01-01").ok_or_else(invalid)?;
    if day < first || day > CalendarDay::of(chrono::Utc::now()) {
        return Err(invalid());
    }
    Ok(day.to_string())
}

pub(super) async fn get_profile(State(state): State<crate::AppState>) -> Result<Json<Profile>> {
    let row = sqlx::query_as!(
        Profile,
        r#"
        SELECT height_cm, sex, hr_max_bpm, birth_date AS "birth_date?",
               client_revision AS "client_revision?"
        FROM profile
        WHERE id = 1
        "#
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to fetch profile: {e}")))?;

    // Not-configured response: the single row simply carries null fields.
    Ok(Json(row.unwrap_or(Profile {
        height_cm: None,
        sex: None,
        hr_max_bpm: None,
        birth_date: None,
        client_revision: None,
    })))
}

/// With a `client_revision`, an edit applies only if it is newer than the last one applied. An older
/// or repeated revision is an edit this client has already superseded or delivered, so it changes
/// nothing and answers with the current Profile: a retry after a lost response and a late delivery
/// both end as acknowledged, without restoring an older revision.
pub(super) async fn update_profile(
    State(state): State<crate::AppState>,
    Json(payload): Json<UpdateProfile>,
) -> Result<Json<Profile>> {
    let client_revision = parse_client_revision(payload.client_revision)?;

    if let Some(Some(height)) = payload.height_cm
        && height <= 0.0
    {
        return Err(AppError::Validation(
            "height_cm must be greater than zero".to_string(),
        ));
    }

    if let Some(Some(hr_max)) = payload.hr_max_bpm
        && !HR_MAX_BPM_RANGE.contains(&hr_max)
    {
        return Err(AppError::Validation(
            "hr_max_bpm must be between 100 and 240".to_string(),
        ));
    }

    let birth_date_set = payload.birth_date.is_some();
    let birth_date = payload
        .birth_date
        .flatten()
        .map(|text| parse_birth_date(&text))
        .transpose()?;

    // Clearable upsert: absent field keeps the current value (stays NULL on
    // first create), explicit null sets NULL (documented in api/AGENTS.md).
    // Single statement keeps concurrent first PATCHes atomic.
    let height_set = payload.height_cm.is_some();
    let height_cm = payload.height_cm.flatten();
    let sex_set = payload.sex.is_some();
    let sex = payload.sex.flatten();
    let hr_max_set = payload.hr_max_bpm.is_some();
    let hr_max_bpm = payload.hr_max_bpm.flatten();
    let profile = sqlx::query_as!(
        Profile,
        r#"
        INSERT INTO profile (id, height_cm, sex, hr_max_bpm, birth_date, client_revision)
        VALUES (1, $1, $2, $5, $7, $9)
        ON CONFLICT(id) DO UPDATE SET
            height_cm = CASE WHEN $3 THEN $1 ELSE height_cm END,
            sex = CASE WHEN $4 THEN $2 ELSE sex END,
            hr_max_bpm = CASE WHEN $6 THEN $5 ELSE hr_max_bpm END,
            birth_date = CASE WHEN $8 THEN $7 ELSE birth_date END,
            client_revision = COALESCE($9, client_revision),
            updated_at = CURRENT_TIMESTAMP
        WHERE $9 IS NULL OR client_revision IS NULL OR client_revision < $9
        RETURNING height_cm, sex, hr_max_bpm, birth_date AS "birth_date?",
                  client_revision AS "client_revision?"
        "#,
        height_cm,
        sex,
        height_set,
        sex_set,
        hr_max_bpm,
        hr_max_set,
        birth_date,
        birth_date_set,
        client_revision
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to update profile: {e}")))?;

    // No row: a revision this Profile has already moved past.
    match profile {
        Some(profile) => Ok(Json(profile)),
        None => get_profile(State(state)).await,
    }
}
