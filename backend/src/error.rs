use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("the requested resource was not found")]
    NotFound,
    #[error("{0}")]
    InvalidRequest(String),
    #[error("the service is not ready")]
    ServiceUnavailable,
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
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let error = match self {
            Self::NotFound => "not_found",
            Self::InvalidRequest(_) => "invalid_request",
            Self::ServiceUnavailable => "service_unavailable",
        };

        HttpResponse::build(self.status_code()).json(ErrorResponse {
            error,
            message: self.to_string(),
        })
    }
}
