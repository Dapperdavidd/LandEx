use actix_web::{HttpResponse, Responder, get, web};
use serde::Serialize;

use crate::{error::ApiError, state::AppState};

#[derive(Serialize)]
struct ServiceStatus {
    status: &'static str,
}

#[get("/health")]
pub async fn health() -> impl Responder {
    HttpResponse::Ok().json(ServiceStatus { status: "ok" })
}

#[get("/ready")]
pub async fn readiness(state: web::Data<AppState>) -> Result<impl Responder, ApiError> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.database)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?;

    Ok(HttpResponse::Ok().json(ServiceStatus { status: "ready" }))
}

#[cfg(test)]
mod tests {
    use actix_web::{App, http::StatusCode, test};

    use super::health;

    #[actix_web::test]
    async fn health_endpoint_reports_ok() {
        let app = test::init_service(App::new().service(health)).await;
        let request = test::TestRequest::get().uri("/health").to_request();
        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::OK);

        let body: serde_json::Value = test::read_body_json(response).await;
        assert_eq!(body, serde_json::json!({ "status": "ok" }));
    }
}
