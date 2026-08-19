use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde_json::Value;
use thiserror::Error;

use crate::domain::{ListingStatus, ListingType, LocationKind, PropertyType};

#[derive(Clone, Debug)]
pub struct ProviderLocation {
    pub source_id: String,
    pub parent_source_id: Option<String>,
    pub kind: LocationKind,
    pub name: String,
    pub normalized_name: String,
    pub country_code: String,
    pub administrative_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub population: Option<i64>,
    pub raw_payload: Value,
}

#[derive(Clone, Debug)]
pub struct ProviderProperty {
    pub source_id: String,
    pub location_source_id: String,
    pub property_type: PropertyType,
    pub address_line: Option<String>,
    pub postal_code: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub bedrooms: Option<Decimal>,
    pub bathrooms: Option<Decimal>,
    pub area_sqm: Option<Decimal>,
    pub lot_size_sqm: Option<Decimal>,
    pub year_built: Option<i16>,
    pub attributes: Value,
    pub raw_payload: Value,
}

#[derive(Clone, Debug)]
pub struct ProviderListing {
    pub source_id: String,
    pub property_source_id: String,
    pub listing_type: ListingType,
    pub status: ListingStatus,
    pub price: Decimal,
    pub currency: String,
    pub listed_at: Option<DateTime<Utc>>,
    pub removed_at: Option<DateTime<Utc>>,
    pub source_url: Option<String>,
    pub observed_at: DateTime<Utc>,
    pub raw_payload: Value,
}

#[derive(Clone, Debug, Default)]
pub struct ProviderPage {
    pub locations: Vec<ProviderLocation>,
    pub properties: Vec<ProviderProperty>,
    pub listings: Vec<ProviderListing>,
    pub next_cursor: Option<String>,
}

impl ProviderPage {
    pub fn validate(&self) -> Result<(), IngestionError> {
        for location in &self.locations {
            validate_identifier("location source_id", &location.source_id)?;
            validate_identifier("location name", &location.name)?;
            validate_country_code(&location.country_code)?;
            validate_coordinates(location.latitude, location.longitude)?;
            if location.population.is_some_and(|population| population < 0) {
                return Err(IngestionError::InvalidRecord(
                    "location population cannot be negative".to_owned(),
                ));
            }
        }

        for property in &self.properties {
            validate_identifier("property source_id", &property.source_id)?;
            validate_identifier("property location_source_id", &property.location_source_id)?;
            validate_coordinates(Some(property.latitude), Some(property.longitude))?;
            if property
                .bedrooms
                .is_some_and(|value| value.is_sign_negative())
                || property
                    .bathrooms
                    .is_some_and(|value| value.is_sign_negative())
            {
                return Err(IngestionError::InvalidRecord(
                    "bedroom and bathroom counts cannot be negative".to_owned(),
                ));
            }
            if property
                .area_sqm
                .is_some_and(|value| value <= Decimal::ZERO)
                || property
                    .lot_size_sqm
                    .is_some_and(|value| value <= Decimal::ZERO)
            {
                return Err(IngestionError::InvalidRecord(
                    "property area values must be positive".to_owned(),
                ));
            }
        }

        for listing in &self.listings {
            validate_identifier("listing source_id", &listing.source_id)?;
            validate_identifier("listing property_source_id", &listing.property_source_id)?;
            validate_currency(&listing.currency)?;
            if listing.price.is_sign_negative() {
                return Err(IngestionError::InvalidRecord(
                    "listing price cannot be negative".to_owned(),
                ));
            }
        }

        Ok(())
    }
}

#[async_trait]
pub trait PropertyProvider: Send + Sync {
    fn slug(&self) -> &'static str;

    async fn fetch_page(&self, cursor: Option<&str>) -> Result<ProviderPage, IngestionError>;
}

#[derive(Debug, Error)]
pub enum IngestionError {
    #[error("provider configuration is invalid: {0}")]
    Configuration(String),
    #[error("provider request failed: {0}")]
    Transport(String),
    #[error("provider rate limit reached")]
    RateLimited,
    #[error("provider response is invalid: {0}")]
    InvalidResponse(String),
    #[error("normalized record is invalid: {0}")]
    InvalidRecord(String),
    #[error("normalized record references missing {entity}: {source_id}")]
    MissingReference {
        entity: &'static str,
        source_id: String,
    },
    #[error("provider returned a repeated pagination cursor")]
    RepeatedCursor,
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

fn validate_identifier(field: &str, value: &str) -> Result<(), IngestionError> {
    if value.trim().is_empty() {
        return Err(IngestionError::InvalidRecord(format!(
            "{field} cannot be empty"
        )));
    }
    Ok(())
}

fn validate_country_code(value: &str) -> Result<(), IngestionError> {
    if value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(IngestionError::InvalidRecord(
            "country_code must contain exactly two uppercase letters".to_owned(),
        ));
    }
    Ok(())
}

fn validate_currency(value: &str) -> Result<(), IngestionError> {
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return Err(IngestionError::InvalidRecord(
            "currency must contain exactly three uppercase letters".to_owned(),
        ));
    }
    Ok(())
}

fn validate_coordinates(
    latitude: Option<f64>,
    longitude: Option<f64>,
) -> Result<(), IngestionError> {
    if latitude.is_some_and(|value| !(-90.0..=90.0).contains(&value)) {
        return Err(IngestionError::InvalidRecord(
            "latitude must be between -90 and 90".to_owned(),
        ));
    }
    if longitude.is_some_and(|value| !(-180.0..=180.0).contains(&value)) {
        return Err(IngestionError::InvalidRecord(
            "longitude must be between -180 and 180".to_owned(),
        ));
    }
    if latitude.is_some() != longitude.is_some() {
        return Err(IngestionError::InvalidRecord(
            "latitude and longitude must be provided together".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;
    use serde_json::json;

    use super::{ProviderListing, ProviderPage};
    use crate::domain::{ListingStatus, ListingType};

    #[test]
    fn accepts_a_valid_normalized_page() {
        let page = ProviderPage {
            listings: vec![ProviderListing {
                source_id: "listing-1".to_owned(),
                property_source_id: "property-1".to_owned(),
                listing_type: ListingType::Sale,
                status: ListingStatus::Active,
                price: Decimal::new(250_000, 0),
                currency: "USD".to_owned(),
                listed_at: None,
                removed_at: None,
                source_url: None,
                observed_at: Utc::now(),
                raw_payload: json!({}),
            }],
            ..ProviderPage::default()
        };

        assert!(page.validate().is_ok());
    }

    #[test]
    fn rejects_nonstandard_currency_codes() {
        let page = ProviderPage {
            listings: vec![ProviderListing {
                source_id: "listing-1".to_owned(),
                property_source_id: "property-1".to_owned(),
                listing_type: ListingType::Rent,
                status: ListingStatus::Active,
                price: Decimal::new(2_000, 0),
                currency: "usd".to_owned(),
                listed_at: None,
                removed_at: None,
                source_url: None,
                observed_at: Utc::now(),
                raw_payload: json!({}),
            }],
            ..ProviderPage::default()
        };

        assert!(page.validate().is_err());
    }
}
