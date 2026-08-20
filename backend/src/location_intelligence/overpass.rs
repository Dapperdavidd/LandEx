use std::collections::HashMap;

use chrono::{Duration, Utc};
use reqwest::{Client, Url, header::CONTENT_TYPE};
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::repository::location_intelligence::{
    LocationIntelligenceRepository, NearbyFeatureInput,
};

const DEFAULT_ENDPOINT: &str = "https://overpass-api.de/api/interpreter";
const MAX_RADIUS_METERS: i32 = 2_000;
const CACHE_DAYS: i64 = 7;

#[derive(Clone)]
pub struct OverpassProvider {
    client: Client,
    endpoint: Url,
}

impl OverpassProvider {
    pub fn new() -> Result<Self, OverpassError> {
        Ok(Self {
            client: Client::builder()
                .user_agent("LandEX/0.1 (+https://github.com/Dapperdavidd/LandEx)")
                .timeout(std::time::Duration::from_secs(45))
                .build()
                .map_err(|error| OverpassError::Configuration(error.to_string()))?,
            endpoint: Url::parse(DEFAULT_ENDPOINT)
                .map_err(|error| OverpassError::Configuration(error.to_string()))?,
        })
    }

    async fn nearby(
        &self,
        latitude: f64,
        longitude: f64,
        radius_meters: i32,
    ) -> Result<Vec<NearbyFeatureInput>, OverpassError> {
        let query = build_query(latitude, longitude, radius_meters)?;
        let response = self
            .client
            .post(self.endpoint.clone())
            .header(CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(query)
            .send()
            .await
            .map_err(|error| OverpassError::Transport(error.to_string()))?;
        if response.status().as_u16() == 429 {
            return Err(OverpassError::RateLimited);
        }
        if !response.status().is_success() {
            return Err(OverpassError::Transport(format!(
                "Overpass returned HTTP {}",
                response.status()
            )));
        }
        let bytes = response.bytes().await.map_err(|_| {
            OverpassError::InvalidResponse("unable to read Overpass response".to_owned())
        })?;
        let payload = serde_json::from_slice::<OverpassResponse>(&bytes).map_err(|error| {
            OverpassError::InvalidResponse(format!("unable to decode Overpass JSON: {error}"))
        })?;
        normalize_elements(payload.elements, latitude, longitude, radius_meters)
    }
}

#[derive(Clone)]
pub struct OverpassEnrichmentService {
    pool: PgPool,
    provider: OverpassProvider,
}

impl OverpassEnrichmentService {
    pub fn new(pool: PgPool, provider: OverpassProvider) -> Self {
        Self { pool, provider }
    }

    pub async fn enrich_property(
        &self,
        property_id: Uuid,
        radius_meters: i32,
    ) -> Result<OverpassEnrichmentReport, OverpassError> {
        if !(100..=MAX_RADIUS_METERS).contains(&radius_meters) {
            return Err(OverpassError::Configuration(format!(
                "radius_meters must be between 100 and {MAX_RADIUS_METERS}"
            )));
        }
        let repository = LocationIntelligenceRepository::new(self.pool.clone());
        let coordinates = repository
            .property_coordinates(property_id)
            .await?
            .ok_or(OverpassError::PropertyNotFound)?;
        let (latitude, longitude) = coordinates
            .complete()
            .ok_or(OverpassError::MissingCoordinates)?;

        let attempt_id = reserve_request(&self.pool).await?;
        let features = match self
            .provider
            .nearby(latitude, longitude, radius_meters)
            .await
        {
            Ok(features) => {
                record_outcome(&self.pool, attempt_id, "succeeded").await?;
                features
            }
            Err(error) => {
                let _ = record_outcome(&self.pool, attempt_id, "failed").await;
                return Err(error);
            }
        };
        let expires_at = Utc::now() + Duration::days(CACHE_DAYS);
        repository
            .replace_property_features(property_id, radius_meters, expires_at, &features)
            .await?;

        Ok(OverpassEnrichmentReport {
            property_id,
            radius_meters,
            feature_count: features.len(),
            expires_at,
        })
    }
}

#[derive(Debug, serde::Serialize)]
pub struct OverpassEnrichmentReport {
    pub property_id: Uuid,
    pub radius_meters: i32,
    pub feature_count: usize,
    pub expires_at: chrono::DateTime<Utc>,
}

async fn reserve_request(pool: &PgPool) -> Result<Uuid, OverpassError> {
    let mut transaction = pool.begin().await?;
    sqlx::query(
        "INSERT INTO providers (slug, name) VALUES ('openstreetmap-overpass', 'openstreetmap-overpass') ON CONFLICT (slug) DO UPDATE SET updated_at = NOW()",
    )
    .execute(&mut *transaction)
    .await?;
    let provider_id: Uuid =
        sqlx::query_scalar("SELECT id FROM providers WHERE slug = 'openstreetmap-overpass'")
            .fetch_one(&mut *transaction)
            .await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('overpass-request-budget', 0))")
        .execute(&mut *transaction)
        .await?;
    let attempts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM provider_request_attempts WHERE provider_id = $1 AND requested_at >= NOW() - INTERVAL '1 day'",
    )
    .bind(provider_id)
    .fetch_one(&mut *transaction)
    .await?;
    if attempts >= 25 {
        return Err(OverpassError::RequestLimitReached);
    }
    let id = sqlx::query_scalar(
        "INSERT INTO provider_request_attempts (provider_id, cursor) VALUES ($1, NULL) RETURNING id",
    )
    .bind(provider_id)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(id)
}

async fn record_outcome(pool: &PgPool, attempt_id: Uuid, outcome: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE provider_request_attempts SET outcome = $2 WHERE id = $1")
        .bind(attempt_id)
        .bind(outcome)
        .execute(pool)
        .await?;
    Ok(())
}

fn build_query(latitude: f64, longitude: f64, radius_meters: i32) -> Result<String, OverpassError> {
    if !(-90.0..=90.0).contains(&latitude) || !(-180.0..=180.0).contains(&longitude) {
        return Err(OverpassError::Configuration(
            "coordinates are invalid".to_owned(),
        ));
    }
    if !(100..=MAX_RADIUS_METERS).contains(&radius_meters) {
        return Err(OverpassError::Configuration("radius is invalid".to_owned()));
    }
    let around = format!("around:{radius_meters},{latitude:.7},{longitude:.7}");
    Ok(format!(
        "[out:json][timeout:25];(nwr({around})[amenity~\"school|university|college|kindergarten|hospital|clinic|doctors|pharmacy|bus_station|fuel|marketplace\"];nwr({around})[public_transport];nwr({around})[railway~\"station|halt|tram_stop|subway_entrance\"];nwr({around})[highway=bus_stop];nwr({around})[shop];nwr({around})[leisure~\"park|sports_centre|fitness_centre\"];nwr({around})[aeroway];);out center tags;"
    ))
}

fn normalize_elements(
    elements: Vec<OverpassElement>,
    property_latitude: f64,
    property_longitude: f64,
    radius_meters: i32,
) -> Result<Vec<NearbyFeatureInput>, OverpassError> {
    let mut deduplicated = HashMap::new();
    for element in elements {
        let Some((latitude, longitude)) = element.coordinates() else {
            continue;
        };
        let Some((category, kind)) = classify(&element.tags) else {
            continue;
        };
        let name = element
            .tags
            .get("name")
            .cloned()
            .filter(|name| !name.trim().is_empty());
        let element_type = element.element_type;
        let distance_meters =
            haversine_meters(property_latitude, property_longitude, latitude, longitude);
        if distance_meters > radius_meters {
            continue;
        }
        deduplicated.insert(
            (element_type.clone(), element.id),
            NearbyFeatureInput {
                source_element_type: element_type,
                source_id: element.id,
                category: category.to_owned(),
                kind: kind.to_owned(),
                name,
                latitude,
                longitude,
                tags: json!(element.tags),
                distance_meters,
            },
        );
    }
    Ok(deduplicated.into_values().collect())
}

fn classify(tags: &HashMap<String, String>) -> Option<(&'static str, &'static str)> {
    let amenity = tags.get("amenity").map(String::as_str);
    match amenity {
        Some("school") => return Some(("education", "school")),
        Some("university") => return Some(("education", "university")),
        Some("college") => return Some(("education", "college")),
        Some("kindergarten") => return Some(("education", "kindergarten")),
        Some("hospital") => return Some(("healthcare", "hospital")),
        Some("clinic") => return Some(("healthcare", "clinic")),
        Some("doctors") => return Some(("healthcare", "doctors")),
        Some("pharmacy") => return Some(("healthcare", "pharmacy")),
        Some("bus_station") => return Some(("transport", "bus_station")),
        Some("fuel") => return Some(("infrastructure", "fuel")),
        Some("marketplace") => return Some(("commerce", "marketplace")),
        _ => {}
    }
    if tags.contains_key("public_transport")
        || tags.contains_key("railway")
        || tags.get("highway").is_some_and(|v| v == "bus_stop")
    {
        return Some(("transport", "transit"));
    }
    if tags.contains_key("shop") {
        return Some(("commerce", "shop"));
    }
    if tags.contains_key("leisure") {
        return Some(("leisure", "recreation"));
    }
    tags.contains_key("aeroway")
        .then_some(("transport", "aeroway"))
}

fn haversine_meters(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> i32 {
    let (lat1, lon1, lat2, lon2) = (
        lat1.to_radians(),
        lon1.to_radians(),
        lat2.to_radians(),
        lon2.to_radians(),
    );
    let delta_lat = lat2 - lat1;
    let delta_lon = lon2 - lon1;
    let a =
        (delta_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (delta_lon / 2.0).sin().powi(2);
    (6_371_000.0 * 2.0 * a.sqrt().atan2((1.0 - a).sqrt())).round() as i32
}

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    elements: Vec<OverpassElement>,
}

#[derive(Debug, Deserialize)]
struct OverpassElement {
    #[serde(rename = "type")]
    element_type: String,
    id: i64,
    lat: Option<f64>,
    lon: Option<f64>,
    center: Option<Center>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

impl OverpassElement {
    fn coordinates(&self) -> Option<(f64, f64)> {
        match (self.lat, self.lon) {
            (Some(lat), Some(lon)) => Some((lat, lon)),
            _ => self.center.as_ref().map(|center| (center.lat, center.lon)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Center {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Error)]
pub enum OverpassError {
    #[error("Overpass configuration is invalid: {0}")]
    Configuration(String),
    #[error("Overpass request failed: {0}")]
    Transport(String),
    #[error("Overpass rate limit reached")]
    RateLimited,
    #[error("Overpass response is invalid: {0}")]
    InvalidResponse(String),
    #[error("property does not exist")]
    PropertyNotFound,
    #[error("property has no coordinates to enrich")]
    MissingCoordinates,
    #[error("LandEX Overpass request guard reached 25 attempts in the last day")]
    RequestLimitReached,
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::{build_query, classify, haversine_meters};
    use std::collections::HashMap;

    #[test]
    fn creates_bounded_query() {
        let query = build_query(6.5244, 3.3792, 1000).expect("query");
        assert!(query.contains("around:1000,6.5244000,3.3792000"));
        assert!(query.contains("[out:json]"));
    }

    #[test]
    fn classifies_common_location_features() {
        assert_eq!(
            classify(&HashMap::from([(
                "amenity".to_owned(),
                "hospital".to_owned()
            )])),
            Some(("healthcare", "hospital"))
        );
        assert_eq!(
            classify(&HashMap::from([(
                "shop".to_owned(),
                "supermarket".to_owned()
            )])),
            Some(("commerce", "shop"))
        );
    }

    #[test]
    fn computes_plausible_haversine_distance() {
        assert_eq!(haversine_meters(6.5244, 3.3792, 6.5244, 3.3792), 0);
        assert!(haversine_meters(6.5244, 3.3792, 6.5334, 3.3792) > 900);
    }
}
