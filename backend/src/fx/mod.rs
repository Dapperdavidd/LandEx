pub mod frankfurter;
pub mod repository;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Clone, Debug)]
pub struct CurrencyRate {
    pub provider: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub rate: Decimal,
    pub observed_on: NaiveDate,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct StoredCurrencyRate {
    pub provider: String,
    pub base_currency: String,
    pub quote_currency: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub rate: Decimal,
    pub observed_on: NaiveDate,
}
