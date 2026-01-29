use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Missing API key")]
    MissingApiKey,
    #[error("Vault unavailable: {0}")]
    VaultUnavailable(String),
    #[error("API key rejected")]
    ApiKeyRejected,
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Biometric authentication failed")]
    BiometricFailed,
    #[error("Biometric unavailable: {0}")]
    BiometricUnavailable(String),
    #[error("Biometric authentication cancelled")]
    BiometricCancelled,
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Config(format!("JSON error: {}", err))
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
