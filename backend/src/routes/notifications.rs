use crate::{
    error::ApiError, repository::notification::NotificationRepository, routes::auth::authenticate,
    state::AppState,
};
use actix_web::{HttpRequest, HttpResponse, delete, get, patch, post, web};
use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateAlertRuleRequest {
    alert_type: String,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    threshold: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAlertRuleRequest {
    enabled: bool,
}

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

#[get("/alert-rules")]
pub async fn list_alert_rules(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    Ok(HttpResponse::Ok().json(
        NotificationRepository::new(state.database.clone())
            .list_rules(user.id)
            .await
            .map_err(|_| ApiError::Internal)?,
    ))
}

#[post("/watchlist-items/{item_id}/alert-rules")]
pub async fn create_alert_rule(
    state: web::Data<AppState>,
    request: HttpRequest,
    item_id: web::Path<Uuid>,
    body: web::Json<CreateAlertRuleRequest>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let alert_type = body.alert_type.trim().to_ascii_lowercase();
    validate_rule(&alert_type, body.threshold)?;
    let rule = NotificationRepository::new(state.database.clone())
        .create_rule(user.id, item_id.into_inner(), &alert_type, body.threshold)
        .await
        .map_err(map_rule_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Created().json(rule))
}

#[patch("/alert-rules/{id}")]
pub async fn update_alert_rule(
    state: web::Data<AppState>,
    request: HttpRequest,
    id: web::Path<Uuid>,
    body: web::Json<UpdateAlertRuleRequest>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let rule = NotificationRepository::new(state.database.clone())
        .set_rule_enabled(user.id, id.into_inner(), body.enabled)
        .await
        .map_err(|_| ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(rule))
}

#[delete("/alert-rules/{id}")]
pub async fn delete_alert_rule(
    state: web::Data<AppState>,
    request: HttpRequest,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    if !NotificationRepository::new(state.database.clone())
        .delete_rule(user.id, id.into_inner())
        .await
        .map_err(|_| ApiError::Internal)?
    {
        return Err(ApiError::NotFound);
    }
    Ok(HttpResponse::NoContent().finish())
}

fn validate_rule(alert_type: &str, threshold: Option<Decimal>) -> Result<(), ApiError> {
    let threshold_required = matches!(
        alert_type,
        "price_change" | "rent_change" | "yield_change" | "market_change"
    );
    let threshold_forbidden = matches!(alert_type, "listing_status_change" | "new_match");
    if !threshold_required && !threshold_forbidden {
        return Err(ApiError::InvalidRequest(
            "unsupported alert_type".to_owned(),
        ));
    }
    if threshold_required
        && threshold.is_none_or(|value| value <= Decimal::ZERO || value > Decimal::from(100))
    {
        return Err(ApiError::InvalidRequest(
            "threshold must be greater than 0 and no more than 100".to_owned(),
        ));
    }
    if threshold_forbidden && threshold.is_some() {
        return Err(ApiError::InvalidRequest(
            "this alert type does not accept a threshold".to_owned(),
        ));
    }
    Ok(())
}

fn map_rule_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.is_unique_violation()
    {
        return ApiError::Conflict("this alert rule already exists".to_owned());
    }
    ApiError::Internal
}

#[cfg(test)]
mod tests {
    use super::validate_rule;
    use rust_decimal::Decimal;

    #[test]
    fn validates_threshold_and_event_alerts() {
        assert!(validate_rule("price_change", Some(Decimal::new(5, 0))).is_ok());
        assert!(validate_rule("price_change", None).is_err());
        assert!(validate_rule("listing_status_change", None).is_ok());
        assert!(validate_rule("listing_status_change", Some(Decimal::ONE)).is_err());
        assert!(validate_rule("unknown", None).is_err());
    }
}
