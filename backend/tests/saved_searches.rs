#![cfg(feature = "integration-tests")]

use actix_web::{App, http::header, test, web};
use landex_api::{configure_api, state::AppState};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn saves_and_runs_private_advanced_property_searches(pool: PgPool) {
    seed_investment_property(&pool).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { database: pool }))
            .configure(configure_api),
    )
    .await;
    let registration = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "display_name": "Search Investor",
            "email": "search@example.com",
            "password": "a secure example password"
        }))
        .to_request();
    let registration_response = test::call_service(&app, registration).await;
    let registration_body: Value = test::read_body_json(registration_response).await;
    let token = registration_body["session"]["access_token"]
        .as_str()
        .expect("access token");

    let create = test::TestRequest::post()
        .uri("/api/v1/saved-searches")
        .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
        .set_json(json!({
            "name": "High conviction income",
            "criteria": {
                "country_code": "ng",
                "listing_type": "sale",
                "min_yield_percent": "8",
                "min_growth_percent": "10",
                "min_score": "75"
            }
        }))
        .to_request();
    let create_response = test::call_service(&app, create).await;
    assert_eq!(create_response.status(), 201);
    let created: Value = test::read_body_json(create_response).await;
    let search_id = created["id"].as_str().expect("search id");

    let matches = test::TestRequest::get()
        .uri(&format!("/api/v1/saved-searches/{search_id}/matches"))
        .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
        .to_request();
    let matches_response = test::call_service(&app, matches).await;
    assert_eq!(matches_response.status(), 200);
    let matches_body: Value = test::read_body_json(matches_response).await;
    assert_eq!(matches_body["total"], 1);

    let duplicate = test::TestRequest::post()
        .uri("/api/v1/saved-searches")
        .insert_header((header::AUTHORIZATION, format!("Bearer {token}")))
        .set_json(json!({"name": "HIGH CONVICTION INCOME", "criteria": {}}))
        .to_request();
    assert_eq!(test::call_service(&app, duplicate).await.status(), 409);
}

async fn seed_investment_property(pool: &PgPool) {
    let provider_id: Uuid = sqlx::query_scalar(
        "INSERT INTO providers (slug, name) VALUES ('search-test','Search Test') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("provider");
    let location_id: Uuid = sqlx::query_scalar(
        "INSERT INTO locations (kind, name, normalized_name, country_code, latitude, longitude) VALUES ('city','Lagos','lagos','NG',6.5244,3.3792) RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("location");
    let property_id: Uuid = sqlx::query_scalar(
        "INSERT INTO properties (location_id, property_type, latitude, longitude) VALUES ($1,'apartment',6.5244,3.3792) RETURNING id",
    )
    .bind(location_id)
    .fetch_one(pool)
    .await
    .expect("property");
    sqlx::query(
        "INSERT INTO listings (property_id, provider_id, source_id, listing_type, status, price, currency) VALUES ($1,$2,'sale-1','sale','active',50000000,'NGN')",
    )
    .bind(property_id)
    .bind(provider_id)
    .execute(pool)
    .await
    .expect("listing");
    sqlx::query(
        "INSERT INTO property_observations (property_id, provider_id, observed_on, rental_price_monthly, currency) VALUES ($1,$2,CURRENT_DATE,400000,'NGN')",
    )
    .bind(property_id)
    .bind(provider_id)
    .execute(pool)
    .await
    .expect("rent");
    let market_id: Uuid = sqlx::query_scalar(
        "INSERT INTO markets (location_id, name, property_type) VALUES ($1,'Lagos Apartments','apartment') RETURNING id",
    )
    .bind(location_id)
    .fetch_one(pool)
    .await
    .expect("market");
    sqlx::query(
        "INSERT INTO market_observations (market_id, provider_id, observed_on, currency, annual_growth_percent) VALUES ($1,$2,CURRENT_DATE,'NGN',12)",
    )
    .bind(market_id)
    .bind(provider_id)
    .execute(pool)
    .await
    .expect("market observation");
    sqlx::query(
        "INSERT INTO score_observations (property_id, methodology_version, observed_on, overall_score, components) VALUES ($1,'landex-score-v1',CURRENT_DATE,82,'{}')",
    )
    .bind(property_id)
    .execute(pool)
    .await
    .expect("score");
}
