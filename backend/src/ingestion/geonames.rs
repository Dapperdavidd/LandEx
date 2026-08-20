use std::collections::HashSet;

use async_trait::async_trait;
use reqwest::{Client, Url};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, to_value};

use super::{IngestionError, PropertyProvider, ProviderLocation, ProviderPage, RequestBudget};
use crate::domain::LocationKind;

const DEFAULT_ENDPOINT: &str = "https://secure.geonames.org/searchJSON";
const MAX_ROWS: u16 = 1_000;
const MAX_FREE_START_ROW: usize = 5_000;

pub struct GeoNamesProvider {
    client: Client,
    endpoint: Url,
    username: String,
    query: String,
    country_code: Option<String>,
    max_rows: u16,
}

impl GeoNamesProvider {
    pub fn new(
        username: String,
        query: String,
        country_code: Option<String>,
        max_rows: u16,
    ) -> Result<Self, IngestionError> {
        let username = required("username", username)?;
        let query = required("query", query)?;
        let country_code = country_code
            .map(|value| value.trim().to_ascii_uppercase())
            .filter(|value| !value.is_empty());
        if country_code.as_ref().is_some_and(|value| {
            value.len() != 2 || !value.bytes().all(|b| b.is_ascii_alphabetic())
        }) {
            return Err(IngestionError::Configuration(
                "GeoNames country code must contain two letters".to_owned(),
            ));
        }
        if max_rows == 0 || max_rows > MAX_ROWS {
            return Err(IngestionError::Configuration(format!(
                "GeoNames max_rows must be between 1 and {MAX_ROWS}"
            )));
        }

        Ok(Self {
            client: Client::builder()
                .user_agent("LandEX/0.1 (global real-estate research platform)")
                .build()
                .map_err(|error| IngestionError::Configuration(error.to_string()))?,
            endpoint: Url::parse(DEFAULT_ENDPOINT)
                .map_err(|error| IngestionError::Configuration(error.to_string()))?,
            username,
            query,
            country_code,
            max_rows,
        })
    }

    fn normalize_response(
        &self,
        response: GeoNamesResponse,
        start_row: usize,
    ) -> Result<ProviderPage, IngestionError> {
        if let Some(status) = response.status {
            return Err(IngestionError::InvalidResponse(format!(
                "GeoNames status {}: {}",
                status.value, status.message
            )));
        }

        let returned = response.geonames.len();
        let mut locations = Vec::new();
        let mut seen = HashSet::new();
        for record in response.geonames {
            let country_code = record.country_code.trim().to_ascii_uppercase();
            let country_id = format!("country:{country_code}");
            push_once(
                &mut locations,
                &mut seen,
                ProviderLocation {
                    source_id: country_id.clone(),
                    parent_source_id: None,
                    kind: LocationKind::Country,
                    name: clean(&record.country_name).unwrap_or_else(|| country_code.clone()),
                    normalized_name: country_code.to_ascii_lowercase(),
                    country_code: country_code.clone(),
                    administrative_code: None,
                    latitude: None,
                    longitude: None,
                    population: None,
                    raw_payload: Value::Null,
                },
            );

            let region_name = clean(&record.admin_name1);
            let region_code = clean(&record.admin_code1);
            let parent_source_id = if let Some(region_name) = region_name {
                let region_key = region_code
                    .as_deref()
                    .unwrap_or(&region_name)
                    .to_ascii_lowercase();
                let region_id = format!("region:{country_code}:{region_key}");
                push_once(
                    &mut locations,
                    &mut seen,
                    ProviderLocation {
                        source_id: region_id.clone(),
                        parent_source_id: Some(country_id.clone()),
                        kind: LocationKind::Region,
                        normalized_name: normalize_name(&region_name),
                        name: region_name,
                        country_code: country_code.clone(),
                        administrative_code: region_code,
                        latitude: None,
                        longitude: None,
                        population: None,
                        raw_payload: Value::Null,
                    },
                );
                region_id
            } else {
                country_id
            };

            let raw_payload = to_value(&record)
                .map_err(|error| IngestionError::InvalidResponse(error.to_string()))?;
            let name = clean(&Some(record.name.clone())).ok_or_else(|| {
                IngestionError::InvalidResponse("GeoNames returned an empty name".to_owned())
            })?;
            push_once(
                &mut locations,
                &mut seen,
                ProviderLocation {
                    source_id: format!("geoname:{}", record.geoname_id),
                    parent_source_id: Some(parent_source_id),
                    kind: LocationKind::City,
                    normalized_name: normalize_name(&name),
                    name,
                    country_code,
                    administrative_code: clean(&record.admin_code1),
                    latitude: Some(record.lat),
                    longitude: Some(record.lng),
                    population: record.population,
                    raw_payload,
                },
            );
        }

        let next_start = start_row.saturating_add(returned);
        let next_cursor = (returned > 0
            && next_start < response.total_results_count
            && next_start <= MAX_FREE_START_ROW)
            .then(|| next_start.to_string());

        Ok(ProviderPage {
            locations,
            next_cursor,
            ..ProviderPage::default()
        })
    }
}

#[async_trait]
impl PropertyProvider for GeoNamesProvider {
    fn slug(&self) -> &'static str {
        "geonames"
    }

    fn request_budget(&self) -> Option<RequestBudget> {
        Some(RequestBudget {
            max_attempts: 100,
            window_days: 1,
        })
    }

    async fn fetch_page(&self, cursor: Option<&str>) -> Result<ProviderPage, IngestionError> {
        let start_row = cursor
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|_| IngestionError::InvalidResponse("invalid GeoNames cursor".to_owned()))?;
        if start_row > MAX_FREE_START_ROW {
            return Err(IngestionError::InvalidResponse(
                "GeoNames cursor exceeds the free-service paging limit".to_owned(),
            ));
        }

        let mut request = self.client.get(self.endpoint.clone()).query(&[
            ("q", self.query.as_str()),
            ("username", self.username.as_str()),
            ("featureClass", "P"),
            ("style", "FULL"),
        ]);
        request = request.query(&[
            ("maxRows", self.max_rows.to_string()),
            ("startRow", start_row.to_string()),
        ]);
        if let Some(country_code) = &self.country_code {
            request = request.query(&[("country", country_code)]);
        }

        let response = request
            .send()
            .await
            .map_err(|error| IngestionError::Transport(error.to_string()))?;
        if response.status().as_u16() == 429 {
            return Err(IngestionError::RateLimited);
        }
        if !response.status().is_success() {
            return Err(IngestionError::Transport(format!(
                "GeoNames returned HTTP {}",
                response.status()
            )));
        }
        let payload = response.bytes().await.map_err(|_| {
            IngestionError::InvalidResponse("unable to read GeoNames response".to_owned())
        })?;
        let payload = serde_json::from_slice::<GeoNamesResponse>(&payload).map_err(|error| {
            IngestionError::InvalidResponse(format!("unable to decode GeoNames JSON: {error}"))
        })?;
        self.normalize_response(payload, start_row)
    }
}

fn required(field: &str, value: String) -> Result<String, IngestionError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return Err(IngestionError::Configuration(format!(
            "GeoNames {field} cannot be empty"
        )));
    }
    Ok(value)
}

fn clean(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn normalize_name(value: &str) -> String {
    value.trim().to_lowercase()
}

fn push_once(
    locations: &mut Vec<ProviderLocation>,
    seen: &mut HashSet<String>,
    location: ProviderLocation,
) {
    if seen.insert(location.source_id.clone()) {
        locations.push(location);
    }
}

#[derive(Debug, Deserialize)]
struct GeoNamesResponse {
    #[serde(default, rename = "totalResultsCount")]
    total_results_count: usize,
    #[serde(default)]
    geonames: Vec<GeoNameRecord>,
    status: Option<GeoNamesStatus>,
}

#[derive(Debug, Deserialize)]
struct GeoNamesStatus {
    message: String,
    value: i32,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct GeoNameRecord {
    #[serde(rename = "geonameId")]
    geoname_id: i64,
    name: String,
    #[serde(deserialize_with = "deserialize_f64")]
    lat: f64,
    #[serde(deserialize_with = "deserialize_f64")]
    lng: f64,
    #[serde(rename = "countryCode")]
    country_code: String,
    #[serde(default, rename = "countryName")]
    country_name: Option<String>,
    #[serde(default, rename = "adminName1")]
    admin_name1: Option<String>,
    #[serde(default, rename = "adminCode1")]
    admin_code1: Option<String>,
    population: Option<i64>,
    #[serde(default, rename = "featureCode")]
    feature_code: Option<String>,
}

fn deserialize_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Number {
        Numeric(f64),
        Text(String),
    }

    match Number::deserialize(deserializer)? {
        Number::Numeric(value) => Ok(value),
        Number::Text(value) => value.parse().map_err(serde::de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::{GeoNamesProvider, GeoNamesResponse};

    #[test]
    fn builds_country_region_city_hierarchy_without_duplicates() {
        let provider = GeoNamesProvider::new(
            "test-user".to_owned(),
            "Lagos".to_owned(),
            Some("NG".to_owned()),
            100,
        )
        .expect("provider");
        let response: GeoNamesResponse = serde_json::from_str(
            r#"{
              "totalResultsCount": 1,
              "geonames": [{
                "geonameId": 2332459,
                "name": "Lagos",
                "lat": 6.45407,
                "lng": 3.39467,
                "countryCode": "NG",
                "countryName": "Nigeria",
                "adminName1": "Lagos",
                "adminCode1": "05",
                "population": 15388000,
                "featureCode": "PPLA"
              }]
            }"#,
        )
        .expect("fixture");

        let page = provider.normalize_response(response, 0).expect("page");
        assert_eq!(page.locations.len(), 3);
        assert_eq!(page.locations[0].source_id, "country:NG");
        assert_eq!(page.locations[1].source_id, "region:NG:05");
        assert_eq!(page.locations[2].source_id, "geoname:2332459");
        assert_eq!(
            page.locations[2].parent_source_id.as_deref(),
            Some("region:NG:05")
        );
        assert_eq!(page.locations[2].population, Some(15_388_000));
    }
}
