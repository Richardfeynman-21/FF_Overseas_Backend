#![allow(dead_code)]
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce
};
use rand::{RngCore, thread_rng};

/// Encrypts plaintext using AES-256-GCM with a random 12-byte nonce.
/// Returns a hex-encoded string of the [nonce (12 bytes)][ciphertext] payload.
pub fn encrypt(plaintext: &str, key: &[u8; 32]) -> Result<String, anyhow::Error> {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0u8; 12];
    thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| anyhow::anyhow!("AES GCM encryption failed: {}", e))?;
    
    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    
    Ok(hex::encode(combined))
}

/// Decrypts a hex-encoded string of [nonce (12 bytes)][ciphertext] using AES-256-GCM.
/// Returns the decrypted UTF-8 plaintext.
pub fn decrypt(hex_encrypted: &str, key: &[u8; 32]) -> Result<String, anyhow::Error> {
    let combined = hex::decode(hex_encrypted)
        .map_err(|e| anyhow::anyhow!("Hex decoding failed: {}", e))?;
    
    if combined.len() < 12 {
        return Err(anyhow::anyhow!("Ciphertext is too short (must be at least 12 bytes for nonce)"));
    }
    
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let cipher = Aes256Gcm::new(key.into());
    let nonce = Nonce::from_slice(nonce_bytes);
    
    let plaintext_bytes = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("AES GCM decryption failed: {}", e))?;
    
    String::from_utf8(plaintext_bytes)
        .map_err(|e| anyhow::anyhow!("Plaintext is not valid UTF-8: {}", e))
}

/// Parses a 32-byte key from a 64-character hex string.
pub fn parse_key(hex_key: &str) -> Result<[u8; 32], anyhow::Error> {
    let bytes = hex::decode(hex_key)
        .map_err(|e| anyhow::anyhow!("Invalid hex key: {}", e))?;
    
    if bytes.len() != 32 {
        return Err(anyhow::anyhow!("Key must be exactly 32 bytes (64 hex characters)"));
    }
    
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let key = [0u8; 32];
        let plaintext = "Sensitive data like passport numbers!";
        
        let encrypted = encrypt(plaintext, &key).unwrap();
        assert_ne!(plaintext, encrypted);
        
        let decrypted = decrypt(&encrypted, &key).unwrap();
        assert_eq!(plaintext, decrypted);
    }
}
