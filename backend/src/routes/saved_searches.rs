use actix_web::{HttpRequest, HttpResponse, delete, get, post, put, web};
use serde::Deserialize;
use tracing::error;
use uuid::Uuid;

use crate::{
    error::ApiError,
    repository::{property::PropertyRepository, saved_search::SavedSearchRepository},
    routes::{auth::authenticate, properties::PropertySearchQuery},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct SaveSearchRequest {
    name: String,
    criteria: PropertySearchQuery,
}

#[get("/saved-searches")]
pub async fn list_saved_searches(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let searches = SavedSearchRepository::new(state.database.clone())
        .list(user.id)
        .await
        .map_err(database_error)?;
    Ok(HttpResponse::Ok().json(searches))
}

#[post("/saved-searches")]
pub async fn create_saved_search(
    state: web::Data<AppState>,
    request: HttpRequest,
    body: web::Json<SaveSearchRequest>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let name = validate_name(&body.name)?;
    body.criteria.clone().into_filters()?;
    let criteria = serde_json::to_value(&body.criteria).map_err(|_| ApiError::Internal)?;
    let search = SavedSearchRepository::new(state.database.clone())
        .create(user.id, name, criteria)
        .await
        .map_err(map_write_error)?;
    Ok(HttpResponse::Created().json(search))
}

#[put("/saved-searches/{id}")]
pub async fn update_saved_search(
    state: web::Data<AppState>,
    request: HttpRequest,
    id: web::Path<Uuid>,
    body: web::Json<SaveSearchRequest>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let name = validate_name(&body.name)?;
    body.criteria.clone().into_filters()?;
    let criteria = serde_json::to_value(&body.criteria).map_err(|_| ApiError::Internal)?;
    let search = SavedSearchRepository::new(state.database.clone())
        .update(user.id, id.into_inner(), name, criteria)
        .await
        .map_err(map_write_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(search))
}

#[delete("/saved-searches/{id}")]
pub async fn delete_saved_search(
    state: web::Data<AppState>,
    request: HttpRequest,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    if !SavedSearchRepository::new(state.database.clone())
        .delete(user.id, id.into_inner())
        .await
        .map_err(database_error)?
    {
        return Err(ApiError::NotFound);
    }
    Ok(HttpResponse::NoContent().finish())
}

#[get("/saved-searches/{id}/matches")]
pub async fn match_saved_search(
    state: web::Data<AppState>,
    request: HttpRequest,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let saved = SavedSearchRepository::new(state.database.clone())
        .get(user.id, id.into_inner())
        .await
        .map_err(database_error)?
        .ok_or(ApiError::NotFound)?;
    let query: PropertySearchQuery =
        serde_json::from_value(saved.criteria).map_err(|_| ApiError::Internal)?;
    let page = PropertyRepository::new(state.database.clone())
        .search(&query.into_filters()?)
        .await
        .map_err(database_error)?;
    Ok(HttpResponse::Ok().json(page))
}

fn validate_name(name: &str) -> Result<&str, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(ApiError::InvalidRequest(
            "name must contain between 1 and 100 characters".to_owned(),
        ));
    }
    Ok(name)
}

fn map_write_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.is_unique_violation()
    {
        return ApiError::Conflict("a saved search with this name already exists".to_owned());
    }
    database_error(error)
}

fn database_error(error: sqlx::Error) -> ApiError {
    error!(?error, "saved-search database operation failed");
    ApiError::Internal
}
