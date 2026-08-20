use actix_web::{HttpRequest, HttpResponse, delete, get, post, web};
use serde::Deserialize;
use tracing::error;
use uuid::Uuid;

use crate::{
    error::ApiError,
    repository::watchlist::{WatchTarget, WatchlistRepository},
    routes::auth::authenticate,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct CreateWatchlistRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddWatchlistItemRequest {
    target_type: String,
    target_id: Uuid,
}

#[get("/watchlists")]
pub async fn list_watchlists(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let watchlists = WatchlistRepository::new(state.database.clone())
        .list(user.id)
        .await
        .map_err(database_error)?;
    Ok(HttpResponse::Ok().json(watchlists))
}

#[post("/watchlists")]
pub async fn create_watchlist(
    state: web::Data<AppState>,
    request: HttpRequest,
    body: web::Json<CreateWatchlistRequest>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(ApiError::InvalidRequest(
            "name must contain between 1 and 100 characters".to_owned(),
        ));
    }

    let watchlist = WatchlistRepository::new(state.database.clone())
        .create(user.id, name)
        .await
        .map_err(map_create_error)?;
    Ok(HttpResponse::Created().json(watchlist))
}

#[get("/watchlists/{watchlist_id}")]
pub async fn get_watchlist(
    state: web::Data<AppState>,
    request: HttpRequest,
    watchlist_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let watchlist = WatchlistRepository::new(state.database.clone())
        .get(user.id, watchlist_id.into_inner())
        .await
        .map_err(database_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(watchlist))
}

#[post("/watchlists/{watchlist_id}/items")]
pub async fn add_watchlist_item(
    state: web::Data<AppState>,
    request: HttpRequest,
    watchlist_id: web::Path<Uuid>,
    body: web::Json<AddWatchlistItemRequest>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let target = match body.target_type.trim().to_lowercase().as_str() {
        "property" => WatchTarget::Property(body.target_id),
        "market" => WatchTarget::Market(body.target_id),
        "location" => WatchTarget::Location(body.target_id),
        _ => {
            return Err(ApiError::InvalidRequest(
                "target_type must be property, market, or location".to_owned(),
            ));
        }
    };

    let item = WatchlistRepository::new(state.database.clone())
        .add_item(user.id, watchlist_id.into_inner(), target)
        .await
        .map_err(map_item_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Created().json(item))
}

#[delete("/watchlists/{watchlist_id}/items/{item_id}")]
pub async fn remove_watchlist_item(
    state: web::Data<AppState>,
    request: HttpRequest,
    path: web::Path<(Uuid, Uuid)>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let (watchlist_id, item_id) = path.into_inner();
    let deleted = WatchlistRepository::new(state.database.clone())
        .remove_item(user.id, watchlist_id, item_id)
        .await
        .map_err(database_error)?;
    if !deleted {
        return Err(ApiError::NotFound);
    }
    Ok(HttpResponse::NoContent().finish())
}

fn map_create_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.is_unique_violation()
    {
        return ApiError::Conflict("a watchlist with this name already exists".to_owned());
    }
    database_error(error)
}

fn map_item_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.is_unique_violation() {
            return ApiError::Conflict("this item is already in the watchlist".to_owned());
        }
        if database_error.is_foreign_key_violation() {
            return ApiError::NotFound;
        }
    }
    database_error(error)
}

fn database_error(error: sqlx::Error) -> ApiError {
    error!(?error, "watchlist database operation failed");
    ApiError::Internal
}
