use axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};
use serde_json;

use crate::error::AppError;

#[derive(Deserialize)]
struct ExternalFactResponse {
    text: String,
}

#[derive(Serialize)]
struct FactResponse {
    text: String,
}

pub fn router() -> Router<crate::AppState> {
    Router::new().route("/api/facts/random", get(get_random_fact))
}

async fn get_random_fact() -> Result<Json<FactResponse>, AppError> {
    let text = reqwest::get("https://uselessfacts.jsph.pl/api/v2/facts/random")
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch fact: {}", e))?
        .text()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read fact response: {}", e))?;

    let external_fact: ExternalFactResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("Failed to parse fact response: {}", e))?;

    Ok(Json(FactResponse {
        text: external_fact.text,
    }))
}
