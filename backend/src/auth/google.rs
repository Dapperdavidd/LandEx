use std::{
    sync::{OnceLock, RwLock},
    time::{Duration, Instant},
};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;

const GOOGLE_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(3600);

#[derive(Clone, Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Clone, Debug, Deserialize)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
}

#[derive(Clone)]
struct CachedKeys {
    set: JwkSet,
    expires_at: Instant,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GoogleClaims {
    pub sub: String,
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    pub aud: String,
    pub iss: String,
    pub exp: usize,
}

#[derive(Debug, Error)]
pub enum GoogleVerificationError {
    #[error("Google sign-in is not configured")]
    NotConfigured,
    #[error("the Google credential is invalid")]
    InvalidCredential,
    #[error("Google identity verification is temporarily unavailable")]
    Unavailable,
}

static KEY_CACHE: OnceLock<RwLock<Option<CachedKeys>>> = OnceLock::new();

pub async fn verify_google_credential(
    credential: &str,
) -> Result<GoogleClaims, GoogleVerificationError> {
    let client_id = std::env::var("GOOGLE_CLIENT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or(GoogleVerificationError::NotConfigured)?;
    let header =
        decode_header(credential).map_err(|_| GoogleVerificationError::InvalidCredential)?;
    if header.alg != Algorithm::RS256 {
        return Err(GoogleVerificationError::InvalidCredential);
    }
    let kid = header
        .kid
        .ok_or(GoogleVerificationError::InvalidCredential)?;

    let mut keys = cached_keys();
    if !keys.keys.iter().any(|key| key.kid == kid) {
        keys = fetch_keys().await?;
    }
    let key = keys
        .keys
        .iter()
        .find(|key| key.kid == kid)
        .ok_or(GoogleVerificationError::InvalidCredential)?;

    let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)
        .map_err(|_| GoogleVerificationError::InvalidCredential)?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[client_id]);
    validation.set_issuer(&["accounts.google.com", "https://accounts.google.com"]);
    validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);

    let claims = decode::<GoogleClaims>(credential, &decoding_key, &validation)
        .map_err(|_| GoogleVerificationError::InvalidCredential)?
        .claims;
    if claims.sub.is_empty() || claims.email.is_empty() || !claims.email_verified {
        return Err(GoogleVerificationError::InvalidCredential);
    }
    Ok(claims)
}

fn cached_keys() -> JwkSet {
    KEY_CACHE
        .get_or_init(|| RwLock::new(None))
        .read()
        .ok()
        .and_then(|cache| cache.clone())
        .filter(|cache| cache.expires_at > Instant::now())
        .map(|cache| cache.set)
        .unwrap_or(JwkSet { keys: Vec::new() })
}

async fn fetch_keys() -> Result<JwkSet, GoogleVerificationError> {
    let response = Client::new()
        .get(GOOGLE_JWKS_URL)
        .send()
        .await
        .map_err(|_| GoogleVerificationError::Unavailable)?
        .error_for_status()
        .map_err(|_| GoogleVerificationError::Unavailable)?;
    let ttl = response
        .headers()
        .get(reqwest::header::CACHE_CONTROL)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_max_age)
        .unwrap_or(DEFAULT_CACHE_TTL);
    let set = response
        .json::<JwkSet>()
        .await
        .map_err(|_| GoogleVerificationError::Unavailable)?;
    if set.keys.is_empty() {
        return Err(GoogleVerificationError::Unavailable);
    }
    if let Ok(mut cache) = KEY_CACHE.get_or_init(|| RwLock::new(None)).write() {
        *cache = Some(CachedKeys {
            set: set.clone(),
            expires_at: Instant::now() + ttl,
        });
    }
    Ok(set)
}

fn parse_max_age(value: &str) -> Option<Duration> {
    value.split(',').find_map(|directive| {
        directive
            .trim()
            .strip_prefix("max-age=")?
            .parse::<u64>()
            .ok()
            .map(Duration::from_secs)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_google_cache_max_age() {
        assert_eq!(
            parse_max_age("public, max-age=24403, must-revalidate"),
            Some(Duration::from_secs(24403))
        );
        assert_eq!(parse_max_age("no-cache"), None);
    }
}
