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

#[sqlx::test(migrations = "./migrations")]
async fn stores_dated_rates_and_converts_direct_and_inverse_pairs(pool: PgPool) {
    let observed_on = NaiveDate::from_ymd_opt(2026, 8, 20).expect("date");
    CurrencyRateRepository::new(pool.clone())
        .upsert(&[CurrencyRate {
            provider: "frankfurter:CBN".to_owned(),
            base_currency: "USD".to_owned(),
            quote_currency: "NGN".to_owned(),
            rate: Decimal::new(1500, 0),
            observed_on,
        }])
        .await
        .expect("store rate");

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { database: pool }))
            .configure(configure_api),
    )
    .await;

    let direct = test::TestRequest::post()
        .uri("/api/v1/fx/convert")
        .set_json(json!({ "amount": "100", "base": "usd", "quote": "ngn" }))
        .to_request();
    let direct_response = test::call_service(&app, direct).await;
    assert_eq!(direct_response.status(), 200);
    let direct_body: Value = test::read_body_json(direct_response).await;
    assert_eq!(direct_body["converted_amount"], "150000.0000");
    assert_eq!(direct_body["rate"]["observed_on"], "2026-08-20");

    let inverse = test::TestRequest::post()
        .uri("/api/v1/fx/convert")
        .set_json(json!({ "amount": "150000", "base": "NGN", "quote": "USD" }))
        .to_request();
    let inverse_response = test::call_service(&app, inverse).await;
    assert_eq!(inverse_response.status(), 200);
    let inverse_body: Value = test::read_body_json(inverse_response).await;
    assert_eq!(inverse_body["converted_amount"], "100.0000");
    assert_eq!(inverse_body["rate"]["provider"], "frankfurter:CBN:inverse");
}
