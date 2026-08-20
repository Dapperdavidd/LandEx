use crate::{
    error::ApiError, repository::notification::NotificationRepository, routes::auth::authenticate,
    state::AppState,
};
use actix_web::{HttpRequest, HttpResponse, get, patch, web};
use uuid::Uuid;

#[get("/notifications")]
pub async fn list_notifications(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let notifications = NotificationRepository::new(state.database.clone())
        .list(user.id, 100)
        .await
        .map_err(|_| ApiError::Internal)?;
    Ok(HttpResponse::Ok().json(notifications))
}

#[patch("/notifications/{id}/read")]
pub async fn mark_notification_read(
    state: web::Data<AppState>,
    request: HttpRequest,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let notification = NotificationRepository::new(state.database.clone())
        .mark_read(user.id, id.into_inner())
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(notification))
}
