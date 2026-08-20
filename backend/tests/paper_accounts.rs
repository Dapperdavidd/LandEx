#![cfg(feature = "integration-tests")]

use actix_web::{App, http::header, test, web};
use landex_api::{configure_api, state::AppState};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn creates_private_demo_capital_with_an_auditable_ledger(pool: PgPool) {
    let property_id = seed_property(&pool).await;
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

    let buy = test::TestRequest::post()
        .uri(&format!("/api/v1/paper-accounts/{account_id}/orders"))
        .insert_header((header::AUTHORIZATION, format!("Bearer {first_token}")))
        .set_json(json!({
            "property_id": property_id,
            "side": "buy",
            "amount": "20000"
        }))
        .to_request();
    let buy_response = test::call_service(&app, buy).await;
    assert_eq!(buy_response.status(), 201);
    let bought: Value = test::read_body_json(buy_response).await;
    assert_eq!(bought["units"], "0.040000000000");

    let sell = test::TestRequest::post()
        .uri(&format!("/api/v1/paper-accounts/{account_id}/orders"))
        .insert_header((header::AUTHORIZATION, format!("Bearer {first_token}")))
        .set_json(json!({
            "property_id": property_id,
            "side": "sell",
            "units": "0.01"
        }))
        .to_request();
    assert_eq!(test::call_service(&app, sell).await.status(), 201);

    sqlx::query(
        "INSERT INTO property_observations (property_id, observed_on, asking_price, currency) VALUES ($1,CURRENT_DATE + 1,600000,'USD')",
    )
    .bind(property_id)
    .execute(&pool)
    .await
    .expect("updated price observation");

    let detail = test::TestRequest::get()
        .uri(&format!("/api/v1/paper-accounts/{account_id}"))
        .insert_header((header::AUTHORIZATION, format!("Bearer {first_token}")))
        .to_request();
    let detail_response = test::call_service(&app, detail).await;
    assert_eq!(detail_response.status(), 200);
    let detail_body: Value = test::read_body_json(detail_response).await;
    assert_eq!(detail_body["cash_balance"], "85000.0000");
    assert_eq!(detail_body["positions"][0]["units"], "0.030000000000");

    let performance = test::TestRequest::get()
        .uri(&format!("/api/v1/paper-accounts/{account_id}/performance"))
        .insert_header((header::AUTHORIZATION, format!("Bearer {first_token}")))
        .to_request();
    let performance_response = test::call_service(&app, performance).await;
    assert_eq!(performance_response.status(), 200);
    let performance_body: Value = test::read_body_json(performance_response).await;
    assert_eq!(performance_body["total_value"], "103000.0000000000000000");
    assert_eq!(performance_body["total_pnl"], "3000.0000000000000000");
    assert_eq!(performance_body["total_return_percent"], "3.0000");
    assert_eq!(performance_body["positions"][0]["country_code"], "US");
    assert_eq!(
        performance_body["positions"][0]["property_type"],
        "apartment"
    );

    let allocation = test::TestRequest::get()
        .uri(&format!("/api/v1/paper-accounts/{account_id}/allocation"))
        .insert_header((header::AUTHORIZATION, format!("Bearer {first_token}")))
        .to_request();
    let allocation_response = test::call_service(&app, allocation).await;
    assert_eq!(allocation_response.status(), 200);
    let allocation_body: Value = test::read_body_json(allocation_response).await;
    assert_eq!(allocation_body["by_country"][0]["label"], "US");
    assert_eq!(allocation_body["by_country"][0]["percentage"], "100");

    let user_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE primary_email = $1")
        .bind("portfolio@example.com")
        .fetch_one(&pool)
        .await
        .expect("user id");
    let account_uuid = Uuid::parse_str(account_id).expect("account uuid");
    landex_api::repository::paper_account::PaperAccountRepository::new(pool.clone())
        .record_snapshot(user_id, account_uuid)
        .await
        .expect("record snapshot")
        .expect("snapshot");
    let history = test::TestRequest::get()
        .uri(&format!(
            "/api/v1/paper-accounts/{account_id}/performance-history"
        ))
        .insert_header((header::AUTHORIZATION, format!("Bearer {first_token}")))
        .to_request();
    let history_response = test::call_service(&app, history).await;
    assert_eq!(history_response.status(), 200);
    let history_body: Value = test::read_body_json(history_response).await;
    assert_eq!(history_body[0]["total_value"], "103000.00000000");

    let trades = test::TestRequest::get()
        .uri(&format!("/api/v1/paper-accounts/{account_id}/trades"))
        .insert_header((header::AUTHORIZATION, format!("Bearer {first_token}")))
        .to_request();
    let trades_response = test::call_service(&app, trades).await;
    assert_eq!(trades_response.status(), 200);
    let trades_body: Value = test::read_body_json(trades_response).await;
    assert_eq!(trades_body.as_array().expect("trades").len(), 2);

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

async fn seed_property(pool: &PgPool) -> Uuid {
    let location_id: Uuid = sqlx::query_scalar(
        "INSERT INTO locations (kind, name, normalized_name, country_code) VALUES ('city','Austin','austin','US') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .expect("location");
    let property_id: Uuid = sqlx::query_scalar(
        "INSERT INTO properties (location_id, property_type, latitude, longitude) VALUES ($1,'apartment',30.2672,-97.7431) RETURNING id",
    )
    .bind(location_id)
    .fetch_one(pool)
    .await
    .expect("property");
    sqlx::query(
        "INSERT INTO property_observations (property_id, observed_on, asking_price, currency) VALUES ($1,CURRENT_DATE,500000,'USD')",
    )
    .bind(property_id)
    .execute(pool)
    .await
    .expect("price observation");
    property_id
}
