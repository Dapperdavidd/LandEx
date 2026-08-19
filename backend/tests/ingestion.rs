#![cfg(feature = "integration-tests")]

use async_trait::async_trait;
use chrono::Utc;
use landex_api::{
    domain::{ListingStatus, ListingType, LocationKind, PropertyType},
    ingestion::{
        IngestionError, IngestionService, PropertyProvider, ProviderListing, ProviderLocation,
        ProviderPage, ProviderProperty,
    },
};
use rust_decimal::Decimal;
use serde_json::json;
use sqlx::PgPool;

struct FixtureProvider;

#[async_trait]
impl PropertyProvider for FixtureProvider {
    fn slug(&self) -> &'static str {
        "fixture-provider"
    }

    async fn fetch_page(&self, _cursor: Option<&str>) -> Result<ProviderPage, IngestionError> {
        Ok(ProviderPage {
            locations: vec![
                ProviderLocation {
                    source_id: "city:lagos".to_owned(),
                    parent_source_id: Some("region:lagos".to_owned()),
                    kind: LocationKind::City,
                    name: "Lagos".to_owned(),
                    normalized_name: "lagos".to_owned(),
                    country_code: "NG".to_owned(),
                    administrative_code: None,
                    latitude: Some(6.5244),
                    longitude: Some(3.3792),
                    population: None,
                    raw_payload: json!({}),
                },
                ProviderLocation {
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
                },
                ProviderLocation {
                    source_id: "region:lagos".to_owned(),
                    parent_source_id: Some("country:NG".to_owned()),
                    kind: LocationKind::Region,
                    name: "Lagos State".to_owned(),
                    normalized_name: "lagos state".to_owned(),
                    country_code: "NG".to_owned(),
                    administrative_code: Some("LA".to_owned()),
                    latitude: None,
                    longitude: None,
                    population: None,
                    raw_payload: json!({}),
                },
            ],
            properties: vec![ProviderProperty {
                source_id: "property:1".to_owned(),
                location_source_id: "city:lagos".to_owned(),
                property_type: PropertyType::Apartment,
                address_line: Some("1 Marina Road".to_owned()),
                postal_code: None,
                latitude: 6.4541,
                longitude: 3.3947,
                bedrooms: Some(Decimal::new(3, 0)),
                bathrooms: Some(Decimal::new(2, 0)),
                area_sqm: Some(Decimal::new(150, 0)),
                lot_size_sqm: None,
                year_built: Some(2020),
                attributes: json!({}),
                raw_payload: json!({ "fixture": true }),
            }],
            listings: vec![ProviderListing {
                source_id: "listing:1".to_owned(),
                property_source_id: "property:1".to_owned(),
                listing_type: ListingType::Sale,
                status: ListingStatus::Active,
                price: Decimal::new(250_000_000, 0),
                currency: "NGN".to_owned(),
                listed_at: None,
                removed_at: None,
                source_url: None,
                observed_at: Utc::now(),
                raw_payload: json!({ "fixture": true }),
            }],
            next_cursor: None,
        })
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn ingests_an_unordered_provider_page_atomically(pool: PgPool) {
    let report = IngestionService::new(pool.clone())
        .run(&FixtureProvider, 1)
        .await
        .expect("ingestion succeeds");

    assert_eq!(report.pages, 1);
    assert_eq!(report.locations, 3);
    assert_eq!(report.properties, 1);
    assert_eq!(report.listings, 1);

    let location_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM locations")
        .fetch_one(&pool)
        .await
        .expect("count locations");
    let property_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM properties")
        .fetch_one(&pool)
        .await
        .expect("count properties");
    let listing_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM listings")
        .fetch_one(&pool)
        .await
        .expect("count listings");
    let observation_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM property_observations")
        .fetch_one(&pool)
        .await
        .expect("count observations");

    assert_eq!(location_count, 3);
    assert_eq!(property_count, 1);
    assert_eq!(listing_count, 1);
    assert_eq!(observation_count, 1);
}
