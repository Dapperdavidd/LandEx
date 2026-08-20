use actix_web::{HttpResponse, get, web};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::ApiError,
    investment::PropertyScoreInputs,
    repository::{
        location_intelligence::LocationIntelligenceRepository,
        property::{PropertyRepository, PropertySearchFilters},
        score::ScoreRepository,
    },
    state::AppState,
};

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 100;
const DEFAULT_HISTORY_LIMIT: i64 = 60;
const MAX_HISTORY_LIMIT: i64 = 500;
const DEFAULT_LOCATION_RADIUS_METERS: i32 = 1_000;
const MAX_LOCATION_RADIUS_METERS: i32 = 10_000;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PropertySearchQuery {
    pub country_code: Option<String>,
    pub location_id: Option<Uuid>,
    pub property_type: Option<String>,
    pub listing_type: Option<String>,
    pub min_price: Option<Decimal>,
    pub max_price: Option<Decimal>,
    pub currency: Option<String>,
    pub min_yield_percent: Option<Decimal>,
    pub max_yield_percent: Option<Decimal>,
    pub min_growth_percent: Option<Decimal>,
    pub max_growth_percent: Option<Decimal>,
    pub min_score: Option<Decimal>,
    pub max_score: Option<Decimal>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct PropertyHistoryQuery {
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct LocationIntelligenceQuery {
    radius_meters: Option<i32>,
    limit: Option<i64>,
}

impl PropertySearchQuery {
    pub(crate) fn into_filters(self) -> Result<PropertySearchFilters, ApiError> {
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
        if let (Some(min_price), Some(max_price)) = (self.min_price, self.max_price)
            && min_price > max_price
        {
            return Err(ApiError::InvalidRequest(
                "min_price cannot be greater than max_price".to_owned(),
            ));
        }
        validate_range(
            self.min_yield_percent,
            self.max_yield_percent,
            "yield_percent",
            Decimal::ZERO,
            Decimal::new(100, 0),
        )?;
        validate_range(
            self.min_growth_percent,
            self.max_growth_percent,
            "growth_percent",
            Decimal::new(-100, 0),
            Decimal::new(1000, 0),
        )?;
        validate_range(
            self.min_score,
            self.max_score,
            "score",
            Decimal::ZERO,
            Decimal::new(100, 0),
        )?;

        Ok(PropertySearchFilters {
            country_code: normalize_code(self.country_code, 2, "country_code")?,
            location_id: self.location_id,
            property_type: normalize_choice(
                self.property_type,
                &[
                    "apartment",
                    "house",
                    "commercial",
                    "land",
                    "hotel",
                    "retail",
                    "industrial",
                    "other",
                ],
                "property_type",
            )?,
            listing_type: normalize_choice(
                self.listing_type,
                &["sale", "rent", "shortlet"],
                "listing_type",
            )?,
            min_price: self.min_price,
            max_price: self.max_price,
            currency: normalize_code(self.currency, 3, "currency")?,
            min_yield_percent: self.min_yield_percent,
            max_yield_percent: self.max_yield_percent,
            min_growth_percent: self.min_growth_percent,
            max_growth_percent: self.max_growth_percent,
            min_score: self.min_score,
            max_score: self.max_score,
            limit,
            offset,
        })
    }
}

fn validate_range(
    min: Option<Decimal>,
    max: Option<Decimal>,
    field: &str,
    allowed_min: Decimal,
    allowed_max: Decimal,
) -> Result<(), ApiError> {
    if min.is_some_and(|value| value < allowed_min || value > allowed_max)
        || max.is_some_and(|value| value < allowed_min || value > allowed_max)
    {
        return Err(ApiError::InvalidRequest(format!(
            "{field} is outside the supported range"
        )));
    }
    if min.zip(max).is_some_and(|(min, max)| min > max) {
        return Err(ApiError::InvalidRequest(format!(
            "min_{field} cannot be greater than max_{field}"
        )));
    }
    Ok(())
}

#[get("/properties")]
pub async fn list_properties(
    state: web::Data<AppState>,
    query: web::Query<PropertySearchQuery>,
) -> Result<HttpResponse, ApiError> {
    let filters = query.into_inner().into_filters()?;
    let page = PropertyRepository::new(state.database.clone())
        .search(&filters)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?;

    Ok(HttpResponse::Ok().json(page))
}

#[get("/properties/{id}")]
pub async fn get_property(
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let property = PropertyRepository::new(state.database.clone())
        .find_by_id(id.into_inner())
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?
        .ok_or(ApiError::NotFound)?;

    Ok(HttpResponse::Ok().json(property))
}

#[get("/properties/{id}/history")]
pub async fn get_property_history(
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    query: web::Query<PropertyHistoryQuery>,
) -> Result<HttpResponse, ApiError> {
    let limit = query.limit.unwrap_or(DEFAULT_HISTORY_LIMIT);
    if !(1..=MAX_HISTORY_LIMIT).contains(&limit) {
        return Err(ApiError::InvalidRequest(format!(
            "limit must be between 1 and {MAX_HISTORY_LIMIT}"
        )));
    }

    let repository = PropertyRepository::new(state.database.clone());
    if repository
        .find_by_id(*id)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?
        .is_none()
    {
        return Err(ApiError::NotFound);
    }
    let history = repository
        .history(*id, limit)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?;

    Ok(HttpResponse::Ok().json(history))
}

#[get("/properties/{id}/location-intelligence")]
pub async fn get_property_location_intelligence(
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
    query: web::Query<LocationIntelligenceQuery>,
) -> Result<HttpResponse, ApiError> {
    let radius_meters = query
        .radius_meters
        .unwrap_or(DEFAULT_LOCATION_RADIUS_METERS);
    if !(100..=MAX_LOCATION_RADIUS_METERS).contains(&radius_meters) {
        return Err(ApiError::InvalidRequest(format!(
            "radius_meters must be between 100 and {MAX_LOCATION_RADIUS_METERS}"
        )));
    }

    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ApiError::InvalidRequest(format!(
            "limit must be between 1 and {MAX_LIMIT}"
        )));
    }

    let intelligence = LocationIntelligenceRepository::new(state.database.clone())
        .for_property(id.into_inner(), radius_meters, limit)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?
        .ok_or(ApiError::NotFound)?;

    Ok(HttpResponse::Ok().json(intelligence))
}

#[get("/properties/{id}/score")]
pub async fn get_property_score(
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let inputs: PropertyScoreInputs = PropertyRepository::new(state.database.clone())
        .score_inputs(id.into_inner())
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?
        .ok_or(ApiError::NotFound)?;
    let score = inputs
        .calculate()
        .map_err(|error| ApiError::InvalidRequest(error.to_string()))?;
    Ok(HttpResponse::Ok().json(score))
}

#[get("/properties/{id}/score-history")]
pub async fn get_property_score_history(
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let history = ScoreRepository::new(state.database.clone())
        .property_history(id.into_inner(), 60)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?;
    Ok(HttpResponse::Ok().json(history))
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

fn normalize_choice(
    value: Option<String>,
    choices: &[&str],
    field: &str,
) -> Result<Option<String>, ApiError> {
    value
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if !choices.contains(&normalized.as_str()) {
                return Err(ApiError::InvalidRequest(format!(
                    "unsupported {field}: {value}"
                )));
            }
            Ok(normalized)
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::PropertySearchQuery;

    #[test]
    fn normalizes_search_codes() {
        let filters = PropertySearchQuery {
            country_code: Some("ng".to_owned()),
            location_id: None,
            property_type: Some("Apartment".to_owned()),
            listing_type: Some("SALE".to_owned()),
            min_price: None,
            max_price: None,
            currency: Some("ngn".to_owned()),
            limit: None,
            offset: None,
            ..Default::default()
        }
        .into_filters()
        .expect("valid filters");

        assert_eq!(filters.country_code.as_deref(), Some("NG"));
        assert_eq!(filters.property_type.as_deref(), Some("apartment"));
        assert_eq!(filters.listing_type.as_deref(), Some("sale"));
        assert_eq!(filters.currency.as_deref(), Some("NGN"));
        assert_eq!(filters.limit, 20);
        assert_eq!(filters.offset, 0);
    }

    #[test]
    fn rejects_invalid_pagination() {
        let result = PropertySearchQuery {
            country_code: None,
            location_id: None,
            property_type: None,
            listing_type: None,
            min_price: None,
            max_price: None,
            currency: None,
            limit: Some(101),
            offset: None,
            ..Default::default()
        }
        .into_filters();

        assert!(result.is_err());
    }
}
