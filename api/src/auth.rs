use std::sync::Arc;

use axum::{extract::Request, http::header, middleware::Next, response::Response};
use subtle::ConstantTimeEq;
use tracing::warn;

use crate::error::AppError;

const BEARER_PREFIX: &str = "Bearer ";

#[derive(Clone)]
pub struct ApiKey(Arc<String>);

impl ApiKey {
    pub fn new(key: String) -> Self {
        Self(Arc::new(key))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub async fn auth_middleware(
    req: Request,
    next: Next,
    api_key: ApiKey,
) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let key = auth_header
        .filter(|v| v.starts_with(BEARER_PREFIX))
        .map(|v| &v[BEARER_PREFIX.len()..]);

    match key {
        Some(k) => {
            if k.as_bytes().ct_eq(api_key.as_str().as_bytes()).into() {
                Ok(next.run(req).await)
            } else {
                warn!(
                    provided_length = k.len(),
                    expected_length = api_key.len(),
                    "Auth failed: invalid key"
                );
                Err(AppError::Unauthorized(
                    "Missing or invalid API key".to_string(),
                ))
            }
        }
        None => {
            warn!(
                provided_length = 0,
                expected_length = api_key.len(),
                "Auth failed: no authorization header"
            );
            Err(AppError::Unauthorized(
                "Missing or invalid API key".to_string(),
            ))
        }
    }
}
