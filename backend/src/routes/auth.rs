use actix_web::{HttpRequest, HttpResponse, get, post, web};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{
    auth::{email_looks_valid, hash_password, normalize_email, verify_password},
    error::ApiError,
    repository::auth::{AuthRepository, SessionTokens, UserRecord},
    state::AppState,
};

const MINIMUM_PASSWORD_LENGTH: usize = 12;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    display_name: String,
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    refresh_token: String,
}

#[derive(Debug, Serialize)]
struct AuthResponse {
    user: UserRecord,
    session: SessionTokens,
}

#[post("/auth/register")]
pub async fn register(
    state: web::Data<AppState>,
    request: web::Json<RegisterRequest>,
) -> Result<HttpResponse, ApiError> {
    let display_name = request.display_name.trim();
    let email = request.email.trim();
    let normalized_email = normalize_email(email);
    validate_registration(display_name, &normalized_email, &request.password)?;

    let password = request.password.clone();
    let password_hash = web::block(move || hash_password(&password))
        .await
        .map_err(|error| {
            error!(?error, "password hashing worker failed");
            ApiError::Internal
        })?
        .map_err(|error| {
            error!(?error, "password hashing failed");
            ApiError::Internal
        })?;

    let repository = AuthRepository::new(state.database.clone());
    let user = repository
        .create_email_user(display_name, email, &normalized_email, &password_hash)
        .await
        .map_err(map_registration_error)?;
    let session = repository
        .create_session(user.id)
        .await
        .map_err(database_error)?;

    Ok(HttpResponse::Created().json(AuthResponse { user, session }))
}

#[post("/auth/login")]
pub async fn login(
    state: web::Data<AppState>,
    request: web::Json<LoginRequest>,
) -> Result<HttpResponse, ApiError> {
    let repository = AuthRepository::new(state.database.clone());
    let normalized_email = normalize_email(&request.email);
    let identity = repository
        .find_password_identity(&normalized_email)
        .await
        .map_err(database_error)?
        .ok_or(ApiError::Unauthorized)?;

    let password = request.password.clone();
    let password_hash = identity.password_hash;
    let valid = web::block(move || verify_password(&password, &password_hash))
        .await
        .map_err(|error| {
            error!(?error, "password verification worker failed");
            ApiError::Internal
        })?;
    if !valid {
        return Err(ApiError::Unauthorized);
    }

    let user = repository
        .find_active_user(identity.user_id)
        .await
        .map_err(database_error)?
        .ok_or(ApiError::Unauthorized)?;
    let session = repository
        .create_session(user.id)
        .await
        .map_err(database_error)?;

    Ok(HttpResponse::Ok().json(AuthResponse { user, session }))
}

#[post("/auth/refresh")]
pub async fn refresh(
    state: web::Data<AppState>,
    request: web::Json<RefreshRequest>,
) -> Result<HttpResponse, ApiError> {
    let tokens = AuthRepository::new(state.database.clone())
        .rotate_refresh_token(&request.refresh_token)
        .await
        .map_err(database_error)?
        .ok_or(ApiError::Unauthorized)?;
    Ok(HttpResponse::Ok().json(tokens))
}

#[post("/auth/logout")]
pub async fn logout(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let token = bearer_token(&request)?;
    let revoked = AuthRepository::new(state.database.clone())
        .revoke_access_token(token)
        .await
        .map_err(database_error)?;
    if !revoked {
        return Err(ApiError::Unauthorized);
    }
    Ok(HttpResponse::NoContent().finish())
}

#[get("/auth/me")]
pub async fn me(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    Ok(HttpResponse::Ok().json(user))
}

pub async fn authenticate(
    state: &web::Data<AppState>,
    request: &HttpRequest,
) -> Result<UserRecord, ApiError> {
    AuthRepository::new(state.database.clone())
        .authenticate_access_token(bearer_token(request)?)
        .await
        .map_err(database_error)?
        .ok_or(ApiError::Unauthorized)
}

fn bearer_token(request: &HttpRequest) -> Result<&str, ApiError> {
    request
        .headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
        .ok_or(ApiError::Unauthorized)
}

fn validate_registration(
    display_name: &str,
    normalized_email: &str,
    password: &str,
) -> Result<(), ApiError> {
    if display_name.is_empty() || display_name.chars().count() > 100 {
        return Err(ApiError::InvalidRequest(
            "display_name must contain between 1 and 100 characters".to_owned(),
        ));
    }
    if !email_looks_valid(normalized_email) {
        return Err(ApiError::InvalidRequest(
            "email must be a valid address".to_owned(),
        ));
    }
    if password.chars().count() < MINIMUM_PASSWORD_LENGTH {
        return Err(ApiError::InvalidRequest(format!(
            "password must contain at least {MINIMUM_PASSWORD_LENGTH} characters"
        )));
    }
    if password.len() > 1024 {
        return Err(ApiError::InvalidRequest("password is too long".to_owned()));
    }
    Ok(())
}

fn map_registration_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.is_unique_violation()
    {
        return ApiError::Conflict("an account with this email already exists".to_owned());
    }
    database_error(error)
}

fn database_error(error: sqlx::Error) -> ApiError {
    error!(?error, "authentication database operation failed");
    ApiError::Internal
}
