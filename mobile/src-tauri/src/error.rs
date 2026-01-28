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
    #[error("Crypto error: {0}")]
    Crypto(#[from] crate::crypto::CryptoError),
    #[error("Biometric authentication failed")]
    BiometricFailed,
    #[error("Biometric unavailable: {0}")]
    BiometricUnavailable(String),
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

impl From<base64::DecodeError> for AppError {
    fn from(_err: base64::DecodeError) -> Self {
        AppError::Crypto(crate::crypto::CryptoError::InvalidCiphertext)
    }
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
