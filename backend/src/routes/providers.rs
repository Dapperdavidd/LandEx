use actix_web::{HttpResponse, get, web};

use crate::{error::ApiError, repository::provider::ProviderRepository, state::AppState};

#[get("/providers/status")]
pub async fn provider_statuses(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let statuses = ProviderRepository::new(state.database.clone())
        .statuses()
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?;
    Ok(HttpResponse::Ok().json(statuses))
}
