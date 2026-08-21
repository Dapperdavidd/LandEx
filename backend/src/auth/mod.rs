use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub mod google;

pub const ACCESS_TOKEN_MINUTES: i64 = 15;
pub const REFRESH_TOKEN_DAYS: i64 = 30;

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("password hashing failed")]
    Hash,
}

pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordError::Hash)
}

pub fn verify_password(password: &str, encoded_hash: &str) -> bool {
    PasswordHash::new(encoded_hash).ok().is_some_and(|hash| {
        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok()
    })
}

pub fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

pub fn email_looks_valid(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };

    !local.is_empty()
        && !domain.is_empty()
        && domain.contains('.')
        && !email.chars().any(char::is_whitespace)
        && email.len() <= 320
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_verifies_passwords() {
        let hash = hash_password("a strong example password").expect("hash password");
        assert_ne!(hash, "a strong example password");
        assert!(verify_password("a strong example password", &hash));
        assert!(!verify_password("incorrect password", &hash));
    }

    #[test]
    fn creates_unpredictable_tokens_and_stable_hashes() {
        let first = generate_token();
        let second = generate_token();
        assert_ne!(first, second);
        assert_eq!(token_hash(&first), token_hash(&first));
        assert_ne!(token_hash(&first), token_hash(&second));
    }

    #[test]
    fn validates_basic_email_shape() {
        assert!(email_looks_valid("person@example.com"));
        assert!(!email_looks_valid("missing-domain@example"));
        assert!(!email_looks_valid("not-an-email"));
    }
}
