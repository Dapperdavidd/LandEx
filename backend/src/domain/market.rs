use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::property::PropertyType;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Market {
    pub id: Uuid,
    pub location_id: Uuid,
    pub property_type: Option<PropertyType>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MarketObservation {
    pub id: Uuid,
    pub market_id: Uuid,
    pub provider_id: Option<Uuid>,
    pub observed_on: NaiveDate,
    pub currency: String,
    pub median_sale_price: Option<Decimal>,
    pub median_rent_monthly: Option<Decimal>,
    pub gross_yield_percent: Option<Decimal>,
    pub annual_growth_percent: Option<Decimal>,
    pub active_inventory: Option<i32>,
    pub days_on_market: Option<Decimal>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}
