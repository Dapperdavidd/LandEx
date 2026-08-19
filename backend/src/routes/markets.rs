use actix_web::{HttpResponse, get, web};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::ApiError,
    repository::market::{MarketRepository, MarketSearchFilters},
    state::AppState,
};

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 100;
const DEFAULT_HISTORY_LIMIT: i64 = 60;
const MAX_HISTORY_LIMIT: i64 = 500;

#[derive(Debug, Deserialize)]
pub struct MarketSearchQuery {
    country_code: Option<String>,
    location_id: Option<Uuid>,
    property_type: Option<String>,
    currency: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct MarketDetailQuery {
    history_limit: Option<i64>,
}

#[get("/markets")]
pub async fn list_markets(
    state: web::Data<AppState>,
    query: web::Query<MarketSearchQuery>,
) -> Result<HttpResponse, ApiError> {
    let filters = query.into_inner().into_filters()?;
    let page = MarketRepository::new(state.database.clone())
        .search(&filters)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?;

    Ok(HttpResponse::Ok().json(page))
}

#[get("/markets/{id}")]
pub async fn get_market(
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    query: web::Query<MarketDetailQuery>,
) -> Result<HttpResponse, ApiError> {
    let history_limit = query.history_limit.unwrap_or(DEFAULT_HISTORY_LIMIT);
    if !(1..=MAX_HISTORY_LIMIT).contains(&history_limit) {
        return Err(ApiError::InvalidRequest(format!(
            "history_limit must be between 1 and {MAX_HISTORY_LIMIT}"
        )));
    }

    let market = MarketRepository::new(state.database.clone())
        .find_by_id(id.into_inner(), history_limit)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?
        .ok_or(ApiError::NotFound)?;

    Ok(HttpResponse::Ok().json(market))
}

impl MarketSearchQuery {
    fn into_filters(self) -> Result<MarketSearchFilters, ApiError> {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        let offset = self.offset.unwrap_or(0);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(ApiError::InvalidRequest(format!(
                "limit must be between 1 and {MAX_LIMIT}"
            )));
        }
        if offset < 0 {
            return Err(ApiError::InvalidRequest(
                "offset cannot be negative".to_owned(),
            ));
        }

        Ok(MarketSearchFilters {
            country_code: normalize_code(self.country_code, 2, "country_code")?,
            location_id: self.location_id,
            property_type: normalize_property_type(self.property_type)?,
            currency: normalize_code(self.currency, 3, "currency")?,
            limit,
            offset,
        })
    }
}

fn normalize_code(
    value: Option<String>,
    expected_length: usize,
    field: &str,
) -> Result<Option<String>, ApiError> {
    value
        .map(|value| {
            let normalized = value.trim().to_ascii_uppercase();
            if normalized.len() != expected_length
                || !normalized.bytes().all(|byte| byte.is_ascii_alphabetic())
            {
                return Err(ApiError::InvalidRequest(format!(
                    "{field} must contain exactly {expected_length} letters"
                )));
            }
            Ok(normalized)
        })
        .transpose()
}

fn normalize_property_type(value: Option<String>) -> Result<Option<String>, ApiError> {
    const TYPES: &[&str] = &[
        "apartment",
        "house",
        "commercial",
        "land",
        "hotel",
        "retail",
        "industrial",
        "other",
    ];

    value
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if !TYPES.contains(&normalized.as_str()) {
                return Err(ApiError::InvalidRequest(format!(
                    "unsupported property_type: {value}"
                )));
            }
            Ok(normalized)
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::MarketSearchQuery;

    #[test]
    fn normalizes_market_filters() {
        let filters = MarketSearchQuery {
            country_code: Some("ae".to_owned()),
            location_id: None,
            property_type: Some("Apartment".to_owned()),
            currency: Some("aed".to_owned()),
            limit: None,
            offset: None,
        }
        .into_filters()
        .expect("valid filters");

        assert_eq!(filters.country_code.as_deref(), Some("AE"));
        assert_eq!(filters.property_type.as_deref(), Some("apartment"));
        assert_eq!(filters.currency.as_deref(), Some("AED"));
    }
}
