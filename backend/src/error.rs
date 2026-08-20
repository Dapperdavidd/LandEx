use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("the requested resource was not found")]
    NotFound,
    #[error("{0}")]
    InvalidRequest(String),
    #[error("authentication is required")]
    Unauthorized,
    #[error("{0}")]
    Conflict(String),
    #[error("the service is not ready")]
    ServiceUnavailable,
    #[error("an internal error occurred")]
    Internal,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
    message: String,
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let error = match self {
            Self::NotFound => "not_found",
            Self::InvalidRequest(_) => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::Conflict(_) => "conflict",
            Self::ServiceUnavailable => "service_unavailable",
            Self::Internal => "internal_error",
        };

        HttpResponse::build(self.status_code()).json(ErrorResponse {
            error,
            message: self.to_string(),
        })
    }
}
