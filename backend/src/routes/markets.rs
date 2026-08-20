use actix_web::{HttpResponse, get, post, web};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::ApiError,
    fx::repository::CurrencyRateRepository,
    investment::PropertyScoreInputs,
    repository::{
        market::{MarketRepository, MarketSearchFilters},
        score::ScoreRepository,
    },
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

#[derive(Debug, Deserialize)]
pub struct CompareMarketsRequest {
    market_ids: Vec<Uuid>,
    target_currency: String,
}

#[derive(Debug, Serialize)]
pub struct MarketComparison {
    id: Uuid,
    name: String,
    property_type: Option<String>,
    country_code: String,
    source_currency: Option<String>,
    target_currency: String,
    observed_on: Option<NaiveDate>,
    conversion_status: String,
    conversion_rate_date: Option<NaiveDate>,
    #[serde(with = "rust_decimal::serde::str_option")]
    median_sale_price: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    median_rent_monthly: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    gross_yield_percent: Option<Decimal>,
    #[serde(with = "rust_decimal::serde::str_option")]
    annual_growth_percent: Option<Decimal>,
    active_inventory: Option<i32>,
    #[serde(with = "rust_decimal::serde::str_option")]
    days_on_market: Option<Decimal>,
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

#[post("/markets/compare")]
pub async fn compare_markets(
    state: web::Data<AppState>,
    body: web::Json<CompareMarketsRequest>,
) -> Result<HttpResponse, ApiError> {
    let target_currency = normalize_code(Some(body.target_currency.clone()), 3, "target_currency")?
        .expect("required target currency");
    let mut market_ids = body.market_ids.clone();
    market_ids.sort_unstable();
    market_ids.dedup();
    if !(2..=10).contains(&market_ids.len()) {
        return Err(ApiError::InvalidRequest(
            "market_ids must contain 2 to 10 unique markets".to_owned(),
        ));
    }

    let markets = MarketRepository::new(state.database.clone());
    let rates = CurrencyRateRepository::new(state.database.clone());
    let mut comparisons = Vec::with_capacity(market_ids.len());
    for market_id in market_ids {
        let market = markets
            .find_by_id(market_id, 1)
            .await
            .map_err(|_| ApiError::ServiceUnavailable)?
            .ok_or(ApiError::NotFound)?;
        let latest = market.history.first();
        let mut sale = latest.and_then(|metric| metric.median_sale_price);
        let mut rent = latest.and_then(|metric| metric.median_rent_monthly);
        let source_currency = latest.map(|metric| metric.currency.clone());
        let observed_on = latest.map(|metric| metric.observed_on);
        let (conversion_status, conversion_rate_date) = match latest {
            None => ("no_market_data".to_owned(), None),
            Some(metric) if metric.currency == target_currency => {
                ("identity".to_owned(), Some(metric.observed_on))
            }
            Some(metric) => match rates
                .latest(&metric.currency, &target_currency, Some(metric.observed_on))
                .await
                .map_err(|_| ApiError::ServiceUnavailable)?
            {
                Some(rate) => {
                    sale = sale.map(|value| (value * rate.rate).round_dp(4));
                    rent = rent.map(|value| (value * rate.rate).round_dp(4));
                    ("converted".to_owned(), Some(rate.observed_on))
                }
                None => {
                    sale = None;
                    rent = None;
                    ("rate_unavailable".to_owned(), None)
                }
            },
        };
        comparisons.push(MarketComparison {
            id: market.id,
            name: market.name,
            property_type: market.property_type,
            country_code: market.country_code,
            source_currency,
            target_currency: target_currency.clone(),
            observed_on,
            conversion_status,
            conversion_rate_date,
            median_sale_price: sale,
            median_rent_monthly: rent,
            gross_yield_percent: latest.and_then(|metric| metric.gross_yield_percent),
            annual_growth_percent: latest.and_then(|metric| metric.annual_growth_percent),
            active_inventory: latest.and_then(|metric| metric.active_inventory),
            days_on_market: latest.and_then(|metric| metric.days_on_market),
        });
    }
    Ok(HttpResponse::Ok().json(comparisons))
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

#[get("/markets/{id}/score")]
pub async fn get_market_score(
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let inputs: PropertyScoreInputs = MarketRepository::new(state.database.clone())
        .score_inputs(id.into_inner())
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?
        .ok_or(ApiError::NotFound)?;
    let score = inputs
        .calculate()
        .map_err(|error| ApiError::InvalidRequest(error.to_string()))?;
    Ok(HttpResponse::Ok().json(score))
}

#[get("/markets/{id}/score-history")]
pub async fn get_market_score_history(
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let history = ScoreRepository::new(state.database.clone())
        .market_history(id.into_inner(), 60)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?;
    Ok(HttpResponse::Ok().json(history))
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
