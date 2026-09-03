#![cfg(feature = "integration-tests")]

use actix_web::{App, http::header, test, web};
use landex_api::{configure_api, state::AppState};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn keeps_watchlists_private_and_supports_each_target_type(pool: PgPool) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState {
                database: pool.clone(),
            }))
            .configure(configure_api),
    )
    .await;

    let first_registration = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "display_name": "First User",
            "email": "first@example.com",
            "password": "a secure example password"
        }))
        .to_request();
    let first_response = test::call_service(&app, first_registration).await;
    assert_eq!(first_response.status(), 201);
    let first_body: Value = test::read_body_json(first_response).await;
    let first_token = first_body["session"]["access_token"]
        .as_str()
        .expect("first access token")
        .to_owned();

    let second_registration = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "display_name": "Second User",
            "email": "second@example.com",
            "password": "a secure example password"
        }))
        .to_request();
    let second_response = test::call_service(&app, second_registration).await;
    assert_eq!(second_response.status(), 201);
    let second_body: Value = test::read_body_json(second_response).await;
    let second_token = second_body["session"]["access_token"]
        .as_str()
        .expect("second access token")
        .to_owned();
    let (property_id, market_id, location_id, instrument_id) = seed_targets(&pool).await;

    let create = authorized(
        test::TestRequest::post().uri("/api/v1/watchlists"),
        &first_token,
    )
    .set_json(json!({ "name": "Global opportunities" }))
    .to_request();
    let create_response = test::call_service(&app, create).await;
    assert_eq!(create_response.status(), 201);
    let created: Value = test::read_body_json(create_response).await;
    let watchlist_id = created["id"].as_str().expect("watchlist id");

    for (target_type, target_id) in [
        ("property", property_id),
        ("market", market_id),
        ("location", location_id),
        ("instrument", instrument_id),
    ] {
        let add = authorized(
            test::TestRequest::post().uri(&format!("/api/v1/watchlists/{watchlist_id}/items")),
            &first_token,
        )
        .set_json(json!({ "target_type": target_type, "target_id": target_id }))
        .to_request();
        assert_eq!(test::call_service(&app, add).await.status(), 201);
    }

    let duplicate = authorized(
        test::TestRequest::post().uri(&format!("/api/v1/watchlists/{watchlist_id}/items")),
        &first_token,
    )
    .set_json(json!({ "target_type": "property", "target_id": property_id }))
    .to_request();
    assert_eq!(test::call_service(&app, duplicate).await.status(), 409);

    let detail = authorized(
        test::TestRequest::get().uri(&format!("/api/v1/watchlists/{watchlist_id}")),
        &first_token,
    )
    .to_request();
    let detail_response = test::call_service(&app, detail).await;
    assert_eq!(detail_response.status(), 200);
    let detail_body: Value = test::read_body_json(detail_response).await;
    assert_eq!(detail_body["item_count"], 4);
    assert_eq!(detail_body["items"].as_array().expect("items").len(), 4);
    assert!(
        detail_body["items"]
            .as_array()
            .expect("items")
            .iter()
            .any(|item| item["instrument_id"] == instrument_id.to_string())
    );

    let private = authorized(
        test::TestRequest::get().uri(&format!("/api/v1/watchlists/{watchlist_id}")),
        &second_token,
    )
    .to_request();
    assert_eq!(test::call_service(&app, private).await.status(), 404);

    let unauthenticated = test::TestRequest::get()
        .uri("/api/v1/watchlists")
        .to_request();
    assert_eq!(
        test::call_service(&app, unauthenticated).await.status(),
        401
    );
}

fn authorized(mut request: test::TestRequest, token: &str) -> test::TestRequest {
    request = request.insert_header((header::AUTHORIZATION, format!("Bearer {token}")));
    request
}

async fn seed_targets(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let location_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO locations (kind, name, normalized_name, country_code)
        VALUES ('city', 'Lagos', 'lagos', 'NG')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("seed location");
    let property_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO properties (location_id, property_type, latitude, longitude)
        VALUES ($1, 'apartment', 6.5244, 3.3792)
        RETURNING id
        "#,
    )
    .bind(location_id)
    .fetch_one(pool)
    .await
    .expect("seed property");
    let market_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO markets (location_id, property_type, name)
        VALUES ($1, 'apartment', 'Lagos Apartment')
        RETURNING id
        "#,
    )
    .bind(location_id)
    .fetch_one(pool)
    .await
    .expect("seed market");
    let instrument_id: Uuid = sqlx::query_scalar(
        "INSERT INTO investment_instruments (slug,name,instrument_kind,status,country_code,currency,valuation_method,liquidity_class) VALUES ('test-reit','Test REIT','listed_security','research','US','USD','SEC identity','listed') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("seed instrument");
    (property_id, market_id, location_id, instrument_id)
}
