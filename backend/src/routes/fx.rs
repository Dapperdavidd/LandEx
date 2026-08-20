use actix_web::{HttpResponse, get, post, web};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{error::ApiError, fx::repository::CurrencyRateRepository, state::AppState};

#[derive(Debug, Deserialize)]
pub struct RateQuery {
    base: String,
    quote: String,
    date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
pub struct ConversionRequest {
    #[serde(with = "rust_decimal::serde::str")]
    amount: Decimal,
    base: String,
    quote: String,
    date: Option<NaiveDate>,
}

#[derive(Debug, Serialize)]
struct ConversionResponse {
    #[serde(with = "rust_decimal::serde::str")]
    amount: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    converted_amount: Decimal,
    rate: crate::fx::StoredCurrencyRate,
}

#[get("/fx/rates")]
pub async fn get_rate(
    state: web::Data<AppState>,
    query: web::Query<RateQuery>,
) -> Result<HttpResponse, ApiError> {
    let (base, quote) = currencies(&query.base, &query.quote)?;
    let rate = CurrencyRateRepository::new(state.database.clone())
        .latest(&base, &quote, query.date)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(rate))
}

#[post("/fx/convert")]
pub async fn convert(
    state: web::Data<AppState>,
    body: web::Json<ConversionRequest>,
) -> Result<HttpResponse, ApiError> {
    if body.amount < Decimal::ZERO {
        return Err(ApiError::InvalidRequest(
            "amount cannot be negative".to_owned(),
        ));
    }
    let (base, quote) = currencies(&body.base, &body.quote)?;
    let (converted_amount, rate) = CurrencyRateRepository::new(state.database.clone())
        .convert(body.amount, &base, &quote, body.date)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(ConversionResponse {
        amount: body.amount,
        converted_amount,
        rate,
    }))
}

fn currencies(base: &str, quote: &str) -> Result<(String, String), ApiError> {
    Ok((currency(base)?, currency(quote)?))
}

fn currency(value: &str) -> Result<String, ApiError> {
    let value = value.trim().to_uppercase();
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(ApiError::InvalidRequest(
            "currencies must be three-letter ISO codes".to_owned(),
        ));
    }
    Ok(value)
}
