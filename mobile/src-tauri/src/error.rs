use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Missing API key")]
    MissingApiKey,
    #[error("Keyring unavailable")]
    KeyringUnavailable(String),
    #[error("API key rejected")]
    ApiKeyRejected,
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Configuration error: {0}")]
    Config(String),
}

impl From<keyring::Error> for AppError {
    fn from(err: keyring::Error) -> Self {
        match err {
            keyring::Error::NoEntry => AppError::MissingApiKey,
            keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => {
                AppError::KeyringUnavailable(err.to_string())
            }
            _ => AppError::KeyringUnavailable(err.to_string()),
        }
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
