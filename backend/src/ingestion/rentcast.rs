use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode, Url};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    IngestionError, PropertyProvider, ProviderListing, ProviderLocation, ProviderPage,
    ProviderProperty,
};
use crate::domain::{ListingStatus, ListingType, LocationKind, PropertyType};

const DEFAULT_BASE_URL: &str = "https://api.rentcast.io/v1/";
const DEFAULT_PAGE_SIZE: u16 = 100;
const MAX_PAGE_SIZE: u16 = 500;
const SQFT_TO_SQM: Decimal = Decimal::from_parts(9_290_304, 0, 0, false, 8);

pub struct RentCastProvider {
    client: Client,
    api_key: String,
    base_url: Url,
    scope: RentCastScope,
    page_size: u16,
}

impl RentCastProvider {
    pub fn new(api_key: String, scope: RentCastScope) -> Result<Self, IngestionError> {
        Self::with_options(api_key, scope, DEFAULT_BASE_URL, DEFAULT_PAGE_SIZE)
    }

    pub fn with_options(
        api_key: String,
        scope: RentCastScope,
        base_url: &str,
        page_size: u16,
    ) -> Result<Self, IngestionError> {
        if api_key.trim().is_empty() {
            return Err(IngestionError::Configuration(
                "RENTCAST_API_KEY cannot be empty".to_owned(),
            ));
        }
        scope.validate()?;
        if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
            return Err(IngestionError::Configuration(format!(
                "RentCast page size must be between 1 and {MAX_PAGE_SIZE}"
            )));
        }

        let base_url = Url::parse(base_url).map_err(|error| {
            IngestionError::Configuration(format!("invalid RentCast base URL: {error}"))
        })?;

        Ok(Self {
            client: Client::new(),
            api_key,
            base_url,
            scope,
            page_size,
        })
    }

    async fn request_page(
        &self,
        kind: RentCastListingKind,
        offset: u32,
    ) -> Result<Vec<Value>, IngestionError> {
        let endpoint = match kind {
            RentCastListingKind::Sale => "listings/sale",
            RentCastListingKind::Rental => "listings/rental/long-term",
        };
        let url = self.base_url.join(endpoint).map_err(|error| {
            IngestionError::Configuration(format!("invalid RentCast endpoint: {error}"))
        })?;

        let mut request = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .header("X-Api-Key", &self.api_key)
            .query(&[
                ("limit", self.page_size.to_string()),
                ("offset", offset.to_string()),
                ("status", "Active".to_owned()),
            ]);

        if let Some(city) = &self.scope.city {
            request = request.query(&[("city", city)]);
        }
        if let Some(state) = &self.scope.state {
            request = request.query(&[("state", state)]);
        }
        if let Some(zip_code) = &self.scope.zip_code {
            request = request.query(&[("zipCode", zip_code)]);
        }

        let response = request
            .send()
            .await
            .map_err(|error| IngestionError::Transport(error.to_string()))?;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            return Err(IngestionError::RateLimited);
        }
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(IngestionError::Transport(format!(
                "RentCast returned {status}: {body}"
            )));
        }

        response
            .json()
            .await
            .map_err(|error| IngestionError::InvalidResponse(error.to_string()))
    }

    fn normalize_page(
        &self,
        records: Vec<Value>,
        kind: RentCastListingKind,
        offset: u32,
    ) -> Result<ProviderPage, IngestionError> {
        let record_count = records.len();
        let mut locations = HashMap::new();
        let mut properties = Vec::with_capacity(record_count);
        let mut listings = Vec::with_capacity(record_count);

        for raw_payload in records {
            let record: RentCastListing = serde_json::from_value(raw_payload.clone())
                .map_err(|error| IngestionError::InvalidResponse(error.to_string()))?;
            let location_source_id = normalize_locations(&record, &mut locations, &raw_payload);
            let property_type = normalize_property_type(&record.property_type);

            properties.push(ProviderProperty {
                source_id: record.id.clone(),
                location_source_id,
                property_type,
                address_line: Some(record.formatted_address),
                postal_code: Some(record.zip_code),
                latitude: record.latitude,
                longitude: record.longitude,
                bedrooms: record.bedrooms,
                bathrooms: record.bathrooms,
                area_sqm: square_feet_to_square_metres(record.square_footage)?,
                lot_size_sqm: square_feet_to_square_metres(record.lot_size)?,
                year_built: record.year_built,
                attributes: json!({
                    "source_property_type": record.property_type,
                    "source_listing_type": record.listing_type,
                    "hoa_fee_monthly": record.hoa.and_then(|hoa| hoa.fee),
                }),
                raw_payload: raw_payload.clone(),
            });

            listings.push(ProviderListing {
                source_id: format!("{}:{}", kind.as_str(), record.id),
                property_source_id: record.id,
                listing_type: kind.listing_type(),
                status: normalize_status(&record.status),
                price: record.price,
                currency: "USD".to_owned(),
                listed_at: record.listed_date,
                removed_at: record.removed_date,
                source_url: None,
                observed_at: record.last_seen_date.unwrap_or_else(Utc::now),
                raw_payload,
            });
        }

        let next_cursor = if record_count < usize::from(self.page_size) {
            match kind {
                RentCastListingKind::Sale => Some(RentCastCursor {
                    kind: RentCastListingKind::Rental,
                    offset: 0,
                }),
                RentCastListingKind::Rental => None,
            }
        } else {
            Some(RentCastCursor {
                kind,
                offset: offset + u32::from(self.page_size),
            })
        };

        Ok(ProviderPage {
            locations: locations.into_values().collect(),
            properties,
            listings,
            next_cursor: next_cursor.map(|cursor| cursor.encode()),
        })
    }
}

#[async_trait]
impl PropertyProvider for RentCastProvider {
    fn slug(&self) -> &'static str {
        "rentcast"
    }

    async fn fetch_page(&self, cursor: Option<&str>) -> Result<ProviderPage, IngestionError> {
        let cursor = cursor.map_or_else(
            || {
                Ok(RentCastCursor {
                    kind: RentCastListingKind::Sale,
                    offset: 0,
                })
            },
            RentCastCursor::decode,
        )?;
        let records = self.request_page(cursor.kind, cursor.offset).await?;
        self.normalize_page(records, cursor.kind, cursor.offset)
    }
}

#[derive(Clone, Debug)]
pub struct RentCastScope {
    pub city: Option<String>,
    pub state: Option<String>,
    pub zip_code: Option<String>,
}

impl RentCastScope {
    pub fn validate(&self) -> Result<(), IngestionError> {
        if self.city.is_none() && self.state.is_none() && self.zip_code.is_none() {
            return Err(IngestionError::Configuration(
                "RentCast requires a city/state, state, or ZIP-code scope".to_owned(),
            ));
        }
        if self.city.is_some() && self.state.is_none() {
            return Err(IngestionError::Configuration(
                "RentCast city scope also requires a state".to_owned(),
            ));
        }
        if let Some(state) = &self.state
            && (state.len() != 2 || !state.bytes().all(|byte| byte.is_ascii_uppercase()))
        {
            return Err(IngestionError::Configuration(
                "RentCast state must contain two uppercase letters".to_owned(),
            ));
        }
        if let Some(zip_code) = &self.zip_code
            && (zip_code.len() != 5 || !zip_code.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return Err(IngestionError::Configuration(
                "RentCast ZIP code must contain five digits".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
enum RentCastListingKind {
    Sale,
    Rental,
}

impl RentCastListingKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Sale => "sale",
            Self::Rental => "rental",
        }
    }

    const fn listing_type(self) -> ListingType {
        match self {
            Self::Sale => ListingType::Sale,
            Self::Rental => ListingType::Rent,
        }
    }
}

struct RentCastCursor {
    kind: RentCastListingKind,
    offset: u32,
}

impl RentCastCursor {
    fn encode(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.offset)
    }

    fn decode(value: &str) -> Result<Self, IngestionError> {
        let (kind, offset) = value
            .split_once(':')
            .ok_or_else(|| IngestionError::InvalidResponse("invalid RentCast cursor".to_owned()))?;
        let kind = match kind {
            "sale" => RentCastListingKind::Sale,
            "rental" => RentCastListingKind::Rental,
            _ => {
                return Err(IngestionError::InvalidResponse(
                    "invalid RentCast cursor listing type".to_owned(),
                ));
            }
        };
        let offset = offset.parse().map_err(|_| {
            IngestionError::InvalidResponse("invalid RentCast cursor offset".to_owned())
        })?;
        Ok(Self { kind, offset })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RentCastListing {
    id: String,
    formatted_address: String,
    city: String,
    state: String,
    zip_code: String,
    county: Option<String>,
    latitude: f64,
    longitude: f64,
    property_type: String,
    bedrooms: Option<Decimal>,
    bathrooms: Option<Decimal>,
    square_footage: Option<Decimal>,
    lot_size: Option<Decimal>,
    year_built: Option<i16>,
    hoa: Option<RentCastHoa>,
    status: String,
    price: Decimal,
    listing_type: Option<String>,
    listed_date: Option<DateTime<Utc>>,
    removed_date: Option<DateTime<Utc>>,
    last_seen_date: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct RentCastHoa {
    fee: Option<Decimal>,
}

fn normalize_locations(
    record: &RentCastListing,
    locations: &mut HashMap<String, ProviderLocation>,
    raw_payload: &Value,
) -> String {
    let country_id = "country:US".to_owned();
    locations
        .entry(country_id.clone())
        .or_insert_with(|| ProviderLocation {
            source_id: country_id.clone(),
            parent_source_id: None,
            kind: LocationKind::Country,
            name: "United States".to_owned(),
            normalized_name: "united states".to_owned(),
            country_code: "US".to_owned(),
            administrative_code: None,
            latitude: None,
            longitude: None,
            population: None,
            raw_payload: json!({ "source": "rentcast" }),
        });

    let state_id = format!("state:{}", record.state);
    locations
        .entry(state_id.clone())
        .or_insert_with(|| ProviderLocation {
            source_id: state_id.clone(),
            parent_source_id: Some(country_id),
            kind: LocationKind::Region,
            name: record.state.clone(),
            normalized_name: record.state.to_ascii_lowercase(),
            country_code: "US".to_owned(),
            administrative_code: Some(record.state.clone()),
            latitude: None,
            longitude: None,
            population: None,
            raw_payload: json!({ "source": "rentcast" }),
        });

    let city_id = format!("city:{}:{}", record.state, record.city.to_ascii_lowercase());
    locations
        .entry(city_id.clone())
        .or_insert_with(|| ProviderLocation {
            source_id: city_id.clone(),
            parent_source_id: Some(state_id),
            kind: LocationKind::City,
            name: record.city.clone(),
            normalized_name: record.city.trim().to_ascii_lowercase(),
            country_code: "US".to_owned(),
            administrative_code: None,
            latitude: None,
            longitude: None,
            population: None,
            raw_payload: json!({ "source": "rentcast" }),
        });

    let zip_id = format!("zip:{}", record.zip_code);
    locations
        .entry(zip_id.clone())
        .or_insert_with(|| ProviderLocation {
            source_id: zip_id.clone(),
            parent_source_id: Some(city_id),
            kind: LocationKind::District,
            name: record.zip_code.clone(),
            normalized_name: record.zip_code.clone(),
            country_code: "US".to_owned(),
            administrative_code: None,
            latitude: Some(record.latitude),
            longitude: Some(record.longitude),
            population: None,
            raw_payload: json!({
                "county": record.county,
                "listing": raw_payload,
            }),
        });

    zip_id
}

fn normalize_property_type(value: &str) -> PropertyType {
    match value {
        "Condo" | "Apartment" => PropertyType::Apartment,
        "Single Family" | "Townhouse" | "Manufactured" | "Multi-Family" => PropertyType::House,
        "Land" => PropertyType::Land,
        _ => PropertyType::Other,
    }
}

fn normalize_status(value: &str) -> ListingStatus {
    match value {
        "Active" => ListingStatus::Active,
        "Inactive" => ListingStatus::Inactive,
        _ => ListingStatus::Unknown,
    }
}

fn square_feet_to_square_metres(value: Option<Decimal>) -> Result<Option<Decimal>, IngestionError> {
    value
        .map(|value| {
            value.checked_mul(SQFT_TO_SQM).ok_or_else(|| {
                IngestionError::InvalidRecord(
                    "property area exceeded the supported numeric range".to_owned(),
                )
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use serde_json::json;

    use super::{RentCastListingKind, RentCastProvider, RentCastScope};

    fn provider() -> RentCastProvider {
        RentCastProvider::with_options(
            "test-key".to_owned(),
            RentCastScope {
                city: Some("Austin".to_owned()),
                state: Some("TX".to_owned()),
                zip_code: None,
            },
            "https://example.com/v1/",
            100,
        )
        .expect("valid provider")
    }

    #[test]
    fn maps_a_sale_listing_into_the_canonical_contract() {
        let page = provider()
            .normalize_page(
                vec![json!({
                    "id": "3821-Hargis-St,-Austin,-TX-78723",
                    "formattedAddress": "3821 Hargis St, Austin, TX 78723",
                    "city": "Austin",
                    "state": "TX",
                    "zipCode": "78723",
                    "county": "Travis",
                    "latitude": 30.290643,
                    "longitude": -97.701547,
                    "propertyType": "Single Family",
                    "bedrooms": 4,
                    "bathrooms": 2.5,
                    "squareFootage": 2345,
                    "lotSize": 3284,
                    "yearBuilt": 2008,
                    "status": "Active",
                    "price": 899000,
                    "listingType": "Standard",
                    "listedDate": "2024-06-24T00:00:00Z",
                    "lastSeenDate": "2024-09-30T13:11:47.157Z"
                })],
                RentCastListingKind::Sale,
                0,
            )
            .expect("valid normalized page");

        assert_eq!(page.properties.len(), 1);
        assert_eq!(page.listings.len(), 1);
        assert_eq!(page.locations.len(), 4);
        assert_eq!(page.listings[0].currency, "USD");
        assert_eq!(page.listings[0].price, Decimal::new(899_000, 0));
        assert_eq!(page.next_cursor.as_deref(), Some("rental:0"));
        page.validate().expect("valid provider contract");
    }

    #[test]
    fn requires_a_geographic_scope() {
        let result = RentCastScope {
            city: None,
            state: None,
            zip_code: None,
        }
        .validate();

        assert!(result.is_err());
    }
}
