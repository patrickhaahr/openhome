use base64::Engine;
use chacha20poly1305::{
    ChaCha20Poly1305, Key, Nonce,
    aead::{Aead, AeadCore, KeyInit, OsRng},
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

const NONCE_SIZE: usize = 12;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EncryptedPayload {
    pub version: u8,
    pub nonce: String,
    pub ciphertext: String,
}

pub fn generate_master_key() -> Result<[u8; 32], CryptoError> {
    let key = ChaCha20Poly1305::generate_key(&mut OsRng);
    Ok(key.into())
}

pub fn encrypt_api_key(
    api_key: &str,
    master_key: &[u8; 32],
) -> Result<EncryptedPayload, CryptoError> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(master_key));
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, api_key.as_bytes())
        .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

    Ok(EncryptedPayload {
        version: 1,
        nonce: base64::engine::general_purpose::STANDARD.encode(nonce),
        ciphertext: base64::engine::general_purpose::STANDARD.encode(ciphertext),
    })
}

pub fn decrypt_api_key(
    payload: &EncryptedPayload,
    master_key: &[u8; 32],
) -> Result<String, CryptoError> {
    if payload.version != 1 {
        return Err(CryptoError::InvalidVersion(payload.version));
    }

    let nonce_bytes = base64::engine::general_purpose::STANDARD
        .decode(&payload.nonce)
        .map_err(|_| CryptoError::InvalidNonce)?;

    if nonce_bytes.len() != NONCE_SIZE {
        return Err(CryptoError::InvalidNonce);
    }

    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(&payload.ciphertext)
        .map_err(|_| CryptoError::InvalidCiphertext)?;

    let cipher = ChaCha20Poly1305::new(Key::from_slice(master_key));
    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| CryptoError::DecryptionFailed(e.to_string()))?;

    String::from_utf8(plaintext).map_err(|_| CryptoError::InvalidUtf8)
}

pub fn zeroize_string(s: &mut String) {
    s.zeroize();
}

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Invalid payload version: {0}")]
    InvalidVersion(u8),

    #[error("Invalid nonce encoding")]
    InvalidNonce,

    #[error("Invalid ciphertext encoding")]
    InvalidCiphertext,

    #[error("Invalid UTF-8 in decrypted data")]
    InvalidUtf8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_master_key() {
        let key1 = generate_master_key().unwrap();
        let key2 = generate_master_key().unwrap();

        assert_eq!(key1.len(), 32);
        assert_eq!(key2.len(), 32);
        assert_ne!(key1, key2, "Keys should be unique");
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let master_key = generate_master_key().unwrap();
        let api_key = "test_api_key_12345";

        let payload = encrypt_api_key(api_key, &master_key).unwrap();
        let decrypted = decrypt_api_key(&payload, &master_key).unwrap();

        assert_eq!(decrypted, api_key);
    }

    #[test]
    fn test_nonce_length() {
        let master_key = generate_master_key().unwrap();
        let payload = encrypt_api_key("test", &master_key).unwrap();

        let nonce = base64::engine::general_purpose::STANDARD
            .decode(&payload.nonce)
            .unwrap();

        assert_eq!(nonce.len(), NONCE_SIZE);
    }

    #[test]
    fn test_invalid_payload_wrong_key() {
        let key1 = generate_master_key().unwrap();
        let key2 = generate_master_key().unwrap();

        let payload = encrypt_api_key("secret", &key1).unwrap();
        let result = decrypt_api_key(&payload, &key2);

        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_payload_bad_base64() {
        let master_key = generate_master_key().unwrap();
        let payload = EncryptedPayload {
            version: 1,
            nonce: "not-base64!!!".to_string(),
            ciphertext: "also-not-base64".to_string(),
        };

        let result = decrypt_api_key(&payload, &master_key);
        assert!(result.is_err());
    }
}
