use async_trait::async_trait;
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::to_value;

use super::{IngestionError, PropertyProvider, ProviderLocation, ProviderPage, RequestBudget};
use crate::domain::LocationKind;

const ENDPOINT: &str = "https://countries.dev/countries?limit=300&fields=name,alpha2Code,alpha3Code,capital,region,subregion,population,latlng";

pub struct CountriesDevProvider {
    client: Client,
    endpoint: Url,
}

impl CountriesDevProvider {
    pub fn new() -> Result<Self, IngestionError> {
        Ok(Self {
            client: Client::builder()
                .user_agent("LandEX/0.1 (global real-estate research platform)")
                .build()
                .map_err(|error| IngestionError::Configuration(error.to_string()))?,
            endpoint: Url::parse(ENDPOINT)
                .map_err(|error| IngestionError::Configuration(error.to_string()))?,
        })
    }
}

#[async_trait]
impl PropertyProvider for CountriesDevProvider {
    fn slug(&self) -> &'static str {
        "countries-dev"
    }
    fn request_budget(&self) -> Option<RequestBudget> {
        Some(RequestBudget {
            max_attempts: 1,
            window_days: 30,
        })
    }
    async fn fetch_page(&self, _cursor: Option<&str>) -> Result<ProviderPage, IngestionError> {
        let response = self
            .client
            .get(self.endpoint.clone())
            .send()
            .await
            .map_err(|error| IngestionError::Transport(error.to_string()))?;
        if response.status().as_u16() == 429 {
            return Err(IngestionError::RateLimited);
        }
        if !response.status().is_success() {
            return Err(IngestionError::Transport(format!(
                "countries.dev returned HTTP {}",
                response.status()
            )));
        }
        let records: Vec<CountryRecord> = response
            .json()
            .await
            .map_err(|error| IngestionError::InvalidResponse(error.to_string()))?;
        if records.len() < 200 {
            return Err(IngestionError::InvalidResponse(
                "country catalogue contains fewer than 200 records".to_owned(),
            ));
        }
        let mut locations = Vec::with_capacity(records.len());
        for record in records {
            let code = record.alpha2_code.trim().to_ascii_uppercase();
            if code.len() != 2 || !code.bytes().all(|byte| byte.is_ascii_alphabetic()) {
                continue;
            }
            let latitude = record.latlng.first().copied();
            let longitude = record.latlng.get(1).copied();
            let raw_payload = to_value(&record)
                .map_err(|error| IngestionError::InvalidResponse(error.to_string()))?;
            locations.push(ProviderLocation {
                source_id: format!("country:{code}"),
                parent_source_id: None,
                kind: LocationKind::Country,
                normalized_name: code.to_ascii_lowercase(),
                name: record.name.clone(),
                country_code: code,
                administrative_code: record.alpha3_code.clone(),
                latitude,
                longitude,
                population: record.population,
                raw_payload,
            });
        }
        Ok(ProviderPage {
            locations,
            ..ProviderPage::default()
        })
    }
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CountryRecord {
    name: String,
    alpha2_code: String,
    alpha3_code: Option<String>,
    capital: Option<String>,
    region: Option<String>,
    subregion: Option<String>,
    population: Option<i64>,
    #[serde(default)]
    latlng: Vec<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decodes_the_public_country_shape() {
        let record: CountryRecord = serde_json::from_str(r#"{"name":"Nigeria","alpha2Code":"NG","alpha3Code":"NGA","capital":"Abuja","region":"Africa","subregion":"Western Africa","population":206139587,"latlng":[10,8]}"#).unwrap();
        assert_eq!(record.alpha2_code, "NG");
        assert_eq!(record.latlng, vec![10.0, 8.0]);
    }
}
