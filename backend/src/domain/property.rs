use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyType {
    Apartment,
    House,
    Commercial,
    Land,
    Hotel,
    Retail,
    Industrial,
    Other,
}

impl PropertyType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Apartment => "apartment",
            Self::House => "house",
            Self::Commercial => "commercial",
            Self::Land => "land",
            Self::Hotel => "hotel",
            Self::Retail => "retail",
            Self::Industrial => "industrial",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ListingType {
    Sale,
    Rent,
    Shortlet,
}

impl ListingType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sale => "sale",
            Self::Rent => "rent",
            Self::Shortlet => "shortlet",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ListingStatus {
    Active,
    Inactive,
    Pending,
    Sold,
    Rented,
    Withdrawn,
    Expired,
    Unknown,
}

impl ListingStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Pending => "pending",
            Self::Sold => "sold",
            Self::Rented => "rented",
            Self::Withdrawn => "withdrawn",
            Self::Expired => "expired",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Property {
    pub id: Uuid,
    pub location_id: Uuid,
    pub property_type: PropertyType,
    pub address_line: Option<String>,
    pub postal_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub bedrooms: Option<Decimal>,
    pub bathrooms: Option<Decimal>,
    pub area_sqm: Option<Decimal>,
    pub lot_size_sqm: Option<Decimal>,
    pub year_built: Option<i16>,
    pub attributes: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Listing {
    pub id: Uuid,
    pub property_id: Uuid,
    pub provider_id: Uuid,
    pub source_id: String,
    pub listing_type: ListingType,
    pub status: ListingStatus,
    pub price: Decimal,
    pub currency: String,
    pub price_period: String,
    pub listed_at: Option<DateTime<Utc>>,
    pub removed_at: Option<DateTime<Utc>>,
    pub source_url: Option<String>,
    pub raw_payload: Value,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PropertyObservation {
    pub id: Uuid,
    pub property_id: Uuid,
    pub provider_id: Option<Uuid>,
    pub observed_on: NaiveDate,
    pub asking_price: Option<Decimal>,
    pub rental_price_monthly: Option<Decimal>,
    pub shortlet_price_nightly: Option<Decimal>,
    pub estimated_value: Option<Decimal>,
    pub currency: String,
    pub days_on_market: Option<i32>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}
