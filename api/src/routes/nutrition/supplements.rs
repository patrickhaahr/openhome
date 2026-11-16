use crate::AppState;
use crate::services::nutrition::supplements;
use crate::services::nutrition::supplements::{
    AddSupplement, Added, EditSupplement, Supplement, SupplementError, Supplements,
};
use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;

fn supplement_body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, SupplementError> {
    body.map(|Json(body)| body)
        .map_err(|rejection| SupplementError::Validation(rejection.body_text()))
}

pub(super) async fn list_supplements(
    State(state): State<AppState>,
) -> Result<Json<Supplements>, SupplementError> {
    Ok(Json(supplements::supplements(&state.db).await?))
}

pub(super) async fn add_supplement(
    State(state): State<AppState>,
    request: Result<Json<AddSupplement>, JsonRejection>,
) -> Result<(StatusCode, Json<Added>), SupplementError> {
    let added = supplements::add_supplement(&state.db, supplement_body(request)?).await?;
    let status = if added.replayed {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(added)))
}

pub(super) async fn update_supplement(
    State(state): State<AppState>,
    Path(id): Path<String>,
    request: Result<Json<EditSupplement>, JsonRejection>,
) -> Result<Json<Supplement>, SupplementError> {
    Ok(Json(
        supplements::update_supplement(&state.db, &id, supplement_body(request)?).await?,
    ))
}

pub(super) async fn delete_supplement(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, SupplementError> {
    supplements::delete_supplement(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
