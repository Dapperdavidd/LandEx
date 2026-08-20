#![cfg(feature = "integration-tests")]

use actix_web::{App, test, web};
use chrono::{Duration, Utc};
use landex_api::{
    configure_api,
    repository::location_intelligence::{LocationIntelligenceRepository, NearbyFeatureInput},
    state::AppState,
};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn returns_cached_nearby_features_and_category_summaries(pool: PgPool) {
    let location_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO locations (kind, name, normalized_name, country_code, latitude, longitude)
        VALUES ('city', 'Lagos', 'lagos', 'NG', 6.5244, 3.3792)
        RETURNING id
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("insert location");
    let property_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO properties (location_id, property_type, latitude, longitude)
        VALUES ($1, 'apartment', 6.5244, 3.3792)
        RETURNING id
        "#,
    )
    .bind(location_id)
    .fetch_one(&pool)
    .await
    .expect("insert property");
    let feature_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO nearby_features (
            source, source_element_type, source_id, category, kind, name, latitude, longitude
        )
        VALUES ('openstreetmap', 'node', 42, 'transport', 'bus_stop', 'Test Bus Stop', 6.525, 3.38)
        RETURNING id
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("insert feature");
    sqlx::query(
        r#"
        INSERT INTO property_nearby_features (
            property_id, feature_id, distance_meters, query_radius_meters, expires_at
        )
        VALUES ($1, $2, 120, 1000, NOW() + INTERVAL '7 days')
        "#,
    )
    .bind(property_id)
    .bind(feature_id)
    .execute(&pool)
    .await
    .expect("link feature");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { database: pool }))
            .configure(configure_api),
    )
    .await;
    let request = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/properties/{property_id}/location-intelligence?radius_meters=500"
        ))
        .to_request();
    let response = test::call_service(&app, request).await;

    assert_eq!(response.status(), 200);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body["cache"]["populated"], true);
    assert_eq!(body["cache"]["fresh"], true);
    assert_eq!(body["categories"][0]["category"], "transport");
    assert_eq!(body["categories"][0]["feature_count"], 1);
    assert_eq!(body["features"][0]["distance_meters"], 120);
    assert_eq!(body["features"][0]["name"], "Test Bus Stop");
}

#[sqlx::test(migrations = "./migrations")]
async fn atomically_replaces_a_radius_cache_with_normalized_features(pool: PgPool) {
    let location_id: Uuid = sqlx::query_scalar(
        "INSERT INTO locations (kind, name, normalized_name, country_code) VALUES ('city', 'Lagos', 'lagos', 'NG') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("location");
    let property_id: Uuid = sqlx::query_scalar(
        "INSERT INTO properties (location_id, property_type, latitude, longitude) VALUES ($1, 'apartment', 6.5244, 3.3792) RETURNING id",
    )
    .bind(location_id)
    .fetch_one(&pool)
    .await
    .expect("property");
    let repository = LocationIntelligenceRepository::new(pool.clone());
    let expires_at = Utc::now() + Duration::days(7);
    repository
        .replace_property_features(
            property_id,
            1_000,
            expires_at,
            &[NearbyFeatureInput {
                source_element_type: "node".to_owned(),
                source_id: 123,
                category: "education".to_owned(),
                kind: "school".to_owned(),
                name: Some("Test School".to_owned()),
                latitude: 6.525,
                longitude: 3.38,
                tags: serde_json::json!({"amenity": "school"}),
                distance_meters: 120,
            }],
        )
        .await
        .expect("cache feature");
    repository
        .replace_property_features(property_id, 1_000, expires_at, &[])
        .await
        .expect("replace cache");

    let cached: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM property_nearby_features WHERE property_id = $1")
            .bind(property_id)
            .fetch_one(&pool)
            .await
            .expect("count cache");
    assert_eq!(cached, 0);
}
