#![cfg(feature = "integration-tests")]

use actix_web::{App, test, web};
use chrono::NaiveDate;
use landex_api::{
    configure_api,
    fx::{CurrencyRate, repository::CurrencyRateRepository},
    state::AppState,
};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn compares_markets_in_one_currency_without_converting_percentages(pool: PgPool) {
    let lagos = seed_market(&pool, "Lagos", "NG", "NGN", 750_000_000, 4_000_000, 8, 11).await;
    let austin = seed_market(&pool, "Austin", "US", "USD", 500_000, 3_500, 8, 7).await;
    CurrencyRateRepository::new(pool.clone())
        .upsert(&[CurrencyRate {
            provider: "test".to_owned(),
            base_currency: "USD".to_owned(),
            quote_currency: "NGN".to_owned(),
            rate: Decimal::new(1500, 0),
            observed_on: NaiveDate::from_ymd_opt(2026, 8, 20).expect("date"),
        }])
        .await
        .expect("rate");
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { database: pool }))
            .configure(configure_api),
    )
    .await;
    let request = test::TestRequest::post()
        .uri("/api/v1/markets/compare")
        .set_json(json!({
            "market_ids": [lagos, austin],
            "target_currency": "ngn"
        }))
        .to_request();
    let response = test::call_service(&app, request).await;
    assert_eq!(response.status(), 200);
    let body: Value = test::read_body_json(response).await;
    let rows = body.as_array().expect("comparison rows");
    let austin = rows
        .iter()
        .find(|row| row["name"] == "Austin")
        .expect("Austin row");
    assert_eq!(austin["median_sale_price"], "750000000.0000");
    assert_eq!(austin["gross_yield_percent"], "8.0000");
    assert_eq!(austin["conversion_status"], "converted");
}

async fn seed_market(
    pool: &PgPool,
    name: &str,
    country: &str,
    currency: &str,
    sale_price: i64,
    rent: i64,
    yield_percent: i64,
    growth: i64,
) -> Uuid {
    let location_id: Uuid = sqlx::query_scalar(
        "INSERT INTO locations (kind, name, normalized_name, country_code) VALUES ('city',$1,LOWER($1),$2) RETURNING id",
    )
    .bind(name)
    .bind(country)
    .fetch_one(pool)
    .await
    .expect("location");
    let market_id: Uuid = sqlx::query_scalar(
        "INSERT INTO markets (location_id, name, property_type) VALUES ($1,$2,'apartment') RETURNING id",
    )
    .bind(location_id)
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("market");
    sqlx::query(
        "INSERT INTO market_observations (market_id, observed_on, currency, median_sale_price, median_rent_monthly, gross_yield_percent, annual_growth_percent, active_inventory, days_on_market) VALUES ($1,'2026-08-20',$2,$3,$4,$5,$6,100,45)",
    )
    .bind(market_id)
    .bind(currency)
    .bind(Decimal::new(sale_price, 0))
    .bind(Decimal::new(rent, 0))
    .bind(Decimal::new(yield_percent, 0))
    .bind(Decimal::new(growth, 0))
    .execute(pool)
    .await
    .expect("market observation");
    market_id
}
