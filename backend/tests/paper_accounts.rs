#![cfg(feature = "integration-tests")]

use actix_web::{App, http::header, test, web};
use landex_api::{configure_api, state::AppState};
use serde_json::{Value, json};
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn creates_private_demo_capital_with_an_auditable_ledger(pool: PgPool) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { database: pool }))
            .configure(configure_api),
    )
    .await;

    let first_registration = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "display_name": "First Investor",
            "email": "portfolio@example.com",
            "password": "a secure example password"
        }))
        .to_request();
    let first_response = test::call_service(&app, first_registration).await;
    let first_body: Value = test::read_body_json(first_response).await;
    let first_token = first_body["session"]["access_token"]
        .as_str()
        .expect("first access token")
        .to_owned();

    let second_registration = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "display_name": "Second Investor",
            "email": "private@example.com",
            "password": "a secure example password"
        }))
        .to_request();
    let second_response = test::call_service(&app, second_registration).await;
    let second_body: Value = test::read_body_json(second_response).await;
    let second_token = second_body["session"]["access_token"]
        .as_str()
        .expect("second access token")
        .to_owned();

    let create = test::TestRequest::post()
        .uri("/api/v1/paper-accounts")
        .insert_header((header::AUTHORIZATION, format!("Bearer {first_token}")))
        .set_json(json!({}))
        .to_request();
    let create_response = test::call_service(&app, create).await;
    assert_eq!(create_response.status(), 201);
    let created: Value = test::read_body_json(create_response).await;
    assert_eq!(created["name"], "Demo Portfolio");
    assert_eq!(created["base_currency"], "USD");
    assert_eq!(created["cash_balance"], "100000.0000");
    let account_id = created["id"].as_str().expect("account id");

    let detail = test::TestRequest::get()
        .uri(&format!("/api/v1/paper-accounts/{account_id}"))
        .insert_header((header::AUTHORIZATION, format!("Bearer {first_token}")))
        .to_request();
    let detail_response = test::call_service(&app, detail).await;
    assert_eq!(detail_response.status(), 200);
    let detail_body: Value = test::read_body_json(detail_response).await;
    assert_eq!(
        detail_body["cash_ledger"][0]["entry_type"],
        "initial_funding"
    );
    assert_eq!(detail_body["cash_ledger"][0]["amount"], "100000.0000");

    let private = test::TestRequest::get()
        .uri(&format!("/api/v1/paper-accounts/{account_id}"))
        .insert_header((header::AUTHORIZATION, format!("Bearer {second_token}")))
        .to_request();
    assert_eq!(test::call_service(&app, private).await.status(), 404);

    let duplicate = test::TestRequest::post()
        .uri("/api/v1/paper-accounts")
        .insert_header((header::AUTHORIZATION, format!("Bearer {first_token}")))
        .set_json(json!({}))
        .to_request();
    assert_eq!(test::call_service(&app, duplicate).await.status(), 409);
}
