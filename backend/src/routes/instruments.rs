use crate::{
    error::ApiError,
    repository::instrument::{InstrumentFilters, InstrumentRepository},
    state::AppState,
};
use actix_web::{HttpResponse, get, web};
use serde::Deserialize;
use uuid::Uuid;

const MAX_LIMIT: i64 = 100;

#[derive(Deserialize)]
pub struct InstrumentQuery {
    kind: Option<String>,
    status: Option<String>,
    country_code: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
pub struct DetailQuery {
    history_limit: Option<i64>,
}

#[get("/instruments")]
pub async fn list(
    state: web::Data<AppState>,
    query: web::Query<InstrumentQuery>,
) -> Result<HttpResponse, ApiError> {
    let q = query.into_inner();
    let limit = q.limit.unwrap_or(20);
    let offset = q.offset.unwrap_or(0);
    if !(1..=MAX_LIMIT).contains(&limit) || offset < 0 {
        return Err(ApiError::InvalidRequest("invalid pagination".into()));
    }
    let country_code = q.country_code.map(|v| v.trim().to_ascii_uppercase());
    if country_code
        .as_ref()
        .is_some_and(|v| v.len() != 2 || !v.bytes().all(|b| b.is_ascii_alphabetic()))
    {
        return Err(ApiError::InvalidRequest(
            "country_code must be ISO alpha-2".into(),
        ));
    }
    let kinds = [
        "direct_property",
        "listed_security",
        "fractional_offering",
        "market_proxy",
    ];
    if q.kind
        .as_ref()
        .is_some_and(|v| !kinds.contains(&v.as_str()))
    {
        return Err(ApiError::InvalidRequest("invalid instrument kind".into()));
    }
    let statuses = ["research", "paper_tradeable", "real_investible", "inactive"];
    if q.status
        .as_ref()
        .is_some_and(|v| !statuses.contains(&v.as_str()))
    {
        return Err(ApiError::InvalidRequest("invalid instrument status".into()));
    }
    let page = InstrumentRepository::new(state.database.clone())
        .search(&InstrumentFilters {
            kind: q.kind,
            status: q.status,
            country_code,
            limit,
            offset,
        })
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?;
    Ok(HttpResponse::Ok().json(page))
}

#[get("/instruments/{id}")]
pub async fn get(
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    query: web::Query<DetailQuery>,
) -> Result<HttpResponse, ApiError> {
    let limit = query.history_limit.unwrap_or(120);
    if !(1..=1000).contains(&limit) {
        return Err(ApiError::InvalidRequest(
            "history_limit must be between 1 and 1000".into(),
        ));
    }
    let item = InstrumentRepository::new(state.database.clone())
        .find(id.into_inner(), limit)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(item))
}

#[get("/coverage")]
pub async fn coverage(state: web::Data<AppState>) -> Result<HttpResponse, ApiError> {
    let rows = InstrumentRepository::new(state.database.clone())
        .coverage()
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?;
    Ok(HttpResponse::Ok().json(rows))
}
