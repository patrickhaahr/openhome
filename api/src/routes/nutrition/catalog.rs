use crate::AppState;
use crate::services::nutrition::food_database::{Food, FoodDatabase, FoodError, FoodSearch};
use crate::services::nutrition::nova::{ClassifiedFood, ClassifyFood};
use crate::services::nutrition::open_food_facts::OpenFoodFacts;
use crate::services::nutrition::products::{LinkProduct, Linked, ProductError, ProductLookup};
use crate::services::nutrition::{nova, products};
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SearchQuery {
    query: String,
    limit: Option<usize>,
}

pub(super) async fn search_foods(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<Json<FoodSearch>, FoodError> {
    let Query(SearchQuery { query, limit }) = query.map_err(|_| {
        FoodError::Validation(
            "query is required; limit must be a whole number; no other parameters are accepted"
                .into(),
        )
    })?;
    Ok(Json(
        nova::search_foods(&state.db, &foods, &query, limit).await?,
    ))
}

fn food_id(text: &str) -> Result<i64, FoodError> {
    text.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or_else(|| FoodError::Validation("food_id must be a positive whole number".into()))
}

pub(super) async fn food(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Path(id): Path<String>,
) -> Result<Json<Food>, FoodError> {
    Ok(Json(nova::food(&state.db, &foods, food_id(&id)?).await?))
}

/// Sets or changes a Food's NOVA Class.
pub(super) async fn classify_food(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Path(id): Path<String>,
    request: Result<Json<ClassifyFood>, JsonRejection>,
) -> Result<Json<ClassifiedFood>, FoodError> {
    let food_id = food_id(&id)?;
    let Json(request) =
        request.map_err(|rejection| FoodError::Validation(rejection.body_text()))?;
    Ok(Json(
        nova::classify_food(&state.db, &foods, food_id, request).await?,
    ))
}

pub(super) async fn lookup_product(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Extension(labels): Extension<OpenFoodFacts>,
    Path(barcode): Path<String>,
) -> Result<Json<ProductLookup>, ProductError> {
    Ok(Json(
        products::lookup(&state.db, &foods, &labels, &barcode).await?,
    ))
}

pub(super) async fn link_product(
    State(state): State<AppState>,
    Extension(foods): Extension<FoodDatabase>,
    Extension(labels): Extension<OpenFoodFacts>,
    request: Result<Json<LinkProduct>, JsonRejection>,
) -> Result<(StatusCode, Json<Linked>), ProductError> {
    let Json(request) =
        request.map_err(|rejection| ProductError::Validation(rejection.body_text()))?;
    let linked = products::link(&state.db, &foods, &labels, request).await?;
    let status = if linked.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(linked)))
}
