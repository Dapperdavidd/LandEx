use async_trait::async_trait;
use chrono::Utc;
use reqwest::{Client, StatusCode, Url};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    IngestionError, PropertyProvider, ProviderListing, ProviderLocation, ProviderPage,
    ProviderProperty, RequestBudget,
};
use crate::domain::{ListingStatus, ListingType, LocationKind, PropertyType};

const DEFAULT_BASE_URL: &str = "https://api.rayprop.io/functions/v1/complete-api/";
const DEFAULT_PAGE_SIZE: u16 = 100;
const MAX_PAGE_SIZE: u16 = 100;
const INTERNAL_DAILY_LIMIT: u16 = 100;
const KOBO_PER_NAIRA: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

pub struct RayPropProvider {
    client: Client,
    api_key: String,
    base_url: Url,
    city: String,
    page_size: u16,
}

impl RayPropProvider {
    pub fn new(api_key: String, city: String) -> Result<Self, IngestionError> {
        Self::with_options(api_key, city, DEFAULT_BASE_URL, DEFAULT_PAGE_SIZE)
    }

    pub fn with_options(
        api_key: String,
        city: String,
        base_url: &str,
        page_size: u16,
    ) -> Result<Self, IngestionError> {
        if !api_key.starts_with("rp_sandbox_") && !api_key.starts_with("rp_live_") {
            return Err(IngestionError::Configuration(
                "RAYPROP_API_KEY must be a sandbox or live key".to_owned(),
            ));
        }
        let city = clean_text(&city);
        if city.is_empty() {
            return Err(IngestionError::Configuration(
                "RAYPROP_CITY cannot be empty".to_owned(),
            ));
        }
        if !(1..=MAX_PAGE_SIZE).contains(&page_size) {
            return Err(IngestionError::Configuration(format!(
                "RayProp page size must be between 1 and {MAX_PAGE_SIZE}"
            )));
        }
        Ok(Self {
            client: Client::builder()
                .user_agent("LandEX/0.1 shortlet ingestion")
                .build()
                .map_err(|error| IngestionError::Configuration(error.to_string()))?,
            api_key,
            base_url: Url::parse(base_url).map_err(|error| {
                IngestionError::Configuration(format!("invalid RayProp base URL: {error}"))
            })?,
            city,
            page_size,
        })
    }

    async fn request_page(&self, page: u32) -> Result<Value, IngestionError> {
        let url = self.base_url.join("listings").map_err(|error| {
            IngestionError::Configuration(format!("invalid RayProp endpoint: {error}"))
        })?;
        let response = self
            .client
            .get(url)
            .header("Accept", "application/json")
            .header("X-API-Key", &self.api_key)
            .query(&[
                ("city", self.city.clone()),
                ("page", page.to_string()),
                ("limit", self.page_size.to_string()),
            ])
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
                "RayProp returned {status}: {body}"
            )));
        }
        response
            .json()
            .await
            .map_err(|error| IngestionError::InvalidResponse(error.to_string()))
    }

    fn normalize_page(&self, payload: Value) -> Result<ProviderPage, IngestionError> {
        let response: RayPropResponse = serde_json::from_value(payload.clone())
            .map_err(|error| IngestionError::InvalidResponse(error.to_string()))?;
        if !response.success {
            return Err(IngestionError::InvalidResponse(
                "RayProp reported an unsuccessful response".to_owned(),
            ));
        }
        let mut locations = std::collections::HashMap::new();
        let mut properties = Vec::with_capacity(response.data.len());
        let mut listings = Vec::with_capacity(response.data.len());
        for raw in response.data {
            let record: RayPropListing = serde_json::from_value(raw.clone())
                .map_err(|error| IngestionError::InvalidResponse(error.to_string()))?;
            let location_id = add_locations(&record, &mut locations, &raw);
            let source_id = clean_text(&record.unique_listing_id.unwrap_or(record.id));
            let title = clean_text(&record.title);
            let description = record.description.map(|value| clean_text(&value));
            let nightly_price = Decimal::from(record.price_per_night) / KOBO_PER_NAIRA;
            properties.push(ProviderProperty {
                source_id: source_id.clone(),
                location_source_id: location_id,
                property_type: PropertyType::Apartment,
                address_line: None,
                postal_code: None,
                latitude: None,
                longitude: None,
                bedrooms: record.bedrooms.map(Decimal::from),
                bathrooms: record.bathrooms.map(Decimal::from),
                area_sqm: record.property_size_sqm,
                lot_size_sqm: None,
                year_built: None,
                attributes: json!({
                    "title": title,
                    "description": description,
                    "source_category": record.property_category,
                    "max_guests": record.max_guests,
                    "amenities": record.amenities,
                    "minimum_stay_nights": record.minimum_stay,
                    "maximum_stay_nights": record.maximum_stay,
                    "check_in_time": record.check_in_time,
                    "check_out_time": record.check_out_time,
                    "verified": record.verified,
                    "images": record.listing_images,
                    "sandbox": self.api_key.starts_with("rp_sandbox_"),
                }),
                raw_payload: raw.clone(),
            });
            listings.push(ProviderListing {
                source_id: format!("shortlet:{source_id}"),
                property_source_id: source_id,
                listing_type: ListingType::Shortlet,
                status: ListingStatus::Active,
                price: nightly_price,
                currency: record.currency.to_uppercase(),
                listed_at: None,
                removed_at: None,
                source_url: None,
                observed_at: Utc::now(),
                raw_payload: raw,
            });
        }
        let next_cursor = response
            .meta
            .has_more
            .then(|| (response.meta.page + 1).to_string());
        Ok(ProviderPage {
            locations: locations.into_values().collect(),
            properties,
            listings,
            next_cursor,
        })
    }
}

#[async_trait]
impl PropertyProvider for RayPropProvider {
    fn slug(&self) -> &'static str {
        "rayprop"
    }

    fn request_budget(&self) -> Option<RequestBudget> {
        Some(RequestBudget {
            max_attempts: INTERNAL_DAILY_LIMIT,
            window_days: 1,
        })
    }

    async fn fetch_page(&self, cursor: Option<&str>) -> Result<ProviderPage, IngestionError> {
        let page = cursor.unwrap_or("1").parse::<u32>().map_err(|_| {
            IngestionError::InvalidResponse("RayProp cursor is not a page number".to_owned())
        })?;
        let payload = self.request_page(page).await?;
        self.normalize_page(payload)
    }
}

fn add_locations(
    record: &RayPropListing,
    locations: &mut std::collections::HashMap<String, ProviderLocation>,
    raw: &Value,
) -> String {
    let state = clean_text(&record.state);
    let city = clean_text(&record.city);
    let neighborhood = clean_text(&record.neighborhood);
    locations
        .entry("country:NG".to_owned())
        .or_insert_with(|| ProviderLocation {
            source_id: "country:NG".to_owned(),
            parent_source_id: None,
            kind: LocationKind::Country,
            name: "Nigeria".to_owned(),
            normalized_name: "nigeria".to_owned(),
            country_code: "NG".to_owned(),
            administrative_code: None,
            latitude: None,
            longitude: None,
            population: None,
            raw_payload: json!({}),
        });
    let state_id = format!("state:{}", state.to_lowercase());
    locations
        .entry(state_id.clone())
        .or_insert_with(|| ProviderLocation {
            source_id: state_id.clone(),
            parent_source_id: Some("country:NG".to_owned()),
            kind: LocationKind::Region,
            name: state.clone(),
            normalized_name: state.to_lowercase(),
            country_code: "NG".to_owned(),
            administrative_code: None,
            latitude: None,
            longitude: None,
            population: None,
            raw_payload: raw.clone(),
        });
    let city_id = format!("city:{}:{}", state.to_lowercase(), city.to_lowercase());
    locations
        .entry(city_id.clone())
        .or_insert_with(|| ProviderLocation {
            source_id: city_id.clone(),
            parent_source_id: Some(state_id),
            kind: LocationKind::City,
            name: city.clone(),
            normalized_name: city.to_lowercase(),
            country_code: "NG".to_owned(),
            administrative_code: None,
            latitude: None,
            longitude: None,
            population: None,
            raw_payload: raw.clone(),
        });
    if neighborhood.is_empty() {
        return city_id;
    }
    let neighborhood_id = format!(
        "neighborhood:{}:{}",
        city.to_lowercase(),
        neighborhood.to_lowercase()
    );
    locations
        .entry(neighborhood_id.clone())
        .or_insert_with(|| ProviderLocation {
            source_id: neighborhood_id.clone(),
            parent_source_id: Some(city_id),
            kind: LocationKind::Neighborhood,
            name: neighborhood.clone(),
            normalized_name: neighborhood.to_lowercase(),
            country_code: "NG".to_owned(),
            administrative_code: None,
            latitude: None,
            longitude: None,
            population: None,
            raw_payload: raw.clone(),
        });
    neighborhood_id
}

fn clean_text(value: &str) -> String {
    value.chars().filter(|character| {
        !character.is_control() && !matches!(*character,
            '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FEFF}')
    }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Deserialize)]
struct RayPropResponse {
    success: bool,
    data: Vec<Value>,
    meta: RayPropMeta,
}
#[derive(Debug, Deserialize)]
struct RayPropMeta {
    page: u32,
    #[serde(rename = "hasMore")]
    has_more: bool,
}
#[derive(Debug, Deserialize)]
struct RayPropListing {
    id: String,
    unique_listing_id: Option<String>,
    title: String,
    description: Option<String>,
    currency: String,
    price_per_night: i64,
    max_guests: Option<i32>,
    bedrooms: Option<i32>,
    bathrooms: Option<i32>,
    property_category: Option<String>,
    city: String,
    state: String,
    #[serde(default)]
    neighborhood: String,
    #[serde(default)]
    amenities: Vec<String>,
    minimum_stay: Option<i32>,
    maximum_stay: Option<i32>,
    check_in_time: Option<String>,
    check_out_time: Option<String>,
    property_size_sqm: Option<Decimal>,
    verified: Option<bool>,
    #[serde(default)]
    listing_images: Vec<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_shortlet_minor_units_and_removes_invisible_text() {
        let provider = RayPropProvider::with_options(
            "rp_sandbox_1234567890123456".to_owned(),
            "Lagos".to_owned(),
            DEFAULT_BASE_URL,
            1,
        )
        .expect("provider");
        let page = provider
            .normalize_page(json!({
                "success": true,
                "data": [{
                    "id":"PROP-1", "unique_listing_id":"PROP-1",
                    "title":"Lekki\u{200b} Apartment", "description":"Verified test",
                    "currency":"NGN", "price_per_night":6000000, "max_guests":2,
                    "bedrooms":1, "bathrooms":1, "property_category":"shortlet",
                    "city":"Lagos", "state":"Lagos", "neighborhood":"Lekki Phase 1",
                    "amenities":["WiFi"], "minimum_stay":1, "maximum_stay":30,
                    "listing_images":[]
                }],
                "meta":{"page":1,"hasMore":false}
            }))
            .expect("page");
        assert_eq!(page.listings[0].listing_type, ListingType::Shortlet);
        assert_eq!(page.listings[0].price, Decimal::new(60_000, 0));
        assert_eq!(page.properties[0].attributes["title"], "Lekki Apartment");
        assert!(page.properties[0].latitude.is_none());
    }
}
