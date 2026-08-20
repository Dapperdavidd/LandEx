#![cfg(feature = "integration-tests")]

use actix_web::{App, test, web};
use landex_api::{configure_api, repository::provider::ProviderRepository, state::AppState};
use serde_json::Value;
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn reports_provider_attempts_and_staleness_without_exposing_credentials(pool: PgPool) {
    let provider_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO providers (slug, name) VALUES ('test-source','Test Source') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("provider");
    sqlx::query(
        "INSERT INTO provider_request_attempts (provider_id, outcome) VALUES ($1,'succeeded')",
    )
    .bind(provider_id)
    .execute(&pool)
    .await
    .expect("attempt");
    ProviderRepository::new(pool.clone())
        .statuses()
        .await
        .expect("provider statuses query");
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { database: pool }))
            .configure(configure_api),
    )
    .await;
    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri("/api/v1/providers/status")
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), 200);
    let body: Value = test::read_body_json(response).await;
    assert_eq!(body[0]["slug"], "test-source");
    assert_eq!(body[0]["attempts_32d"], 1);
    assert_eq!(body[0]["health"], "stale");
    assert!(body[0].get("endpoint").is_none());
}
