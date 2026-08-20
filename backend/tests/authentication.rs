#![cfg(feature = "integration-tests")]

use actix_web::{App, http::header, test, web};
use landex_api::{configure_api, state::AppState};
use serde_json::{Value, json};
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn completes_the_email_password_session_lifecycle(pool: PgPool) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState {
                database: pool.clone(),
            }))
            .configure(configure_api),
    )
    .await;

    let registration = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "display_name": "Ada Investor",
            "email": " Ada@Example.com ",
            "password": "a long example password"
        }))
        .to_request();
    let registration_response = test::call_service(&app, registration).await;
    assert_eq!(registration_response.status(), 201);
    let registration_body: Value = test::read_body_json(registration_response).await;
    let access_token = registration_body["session"]["access_token"]
        .as_str()
        .expect("registration access token")
        .to_owned();
    let refresh_token = registration_body["session"]["refresh_token"]
        .as_str()
        .expect("registration refresh token")
        .to_owned();
    assert_eq!(
        registration_body["user"]["primary_email"],
        "Ada@Example.com"
    );

    let stored: (String, String) = sqlx::query_as(
        "SELECT primary_email_normalized, password_hash FROM users JOIN user_identities ON users.id = user_identities.user_id",
    )
    .fetch_one(&pool)
    .await
    .expect("stored identity");
    assert_eq!(stored.0, "ada@example.com");
    assert!(stored.1.starts_with("$argon2"));
    assert_ne!(stored.1, "a long example password");

    let duplicate = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "display_name": "Duplicate",
            "email": "ada@example.com",
            "password": "another long password"
        }))
        .to_request();
    assert_eq!(test::call_service(&app, duplicate).await.status(), 409);

    let wrong_password = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({
            "email": "ada@example.com",
            "password": "the wrong password"
        }))
        .to_request();
    assert_eq!(test::call_service(&app, wrong_password).await.status(), 401);

    let login = test::TestRequest::post()
        .uri("/api/v1/auth/login")
        .set_json(json!({
            "email": "ADA@example.com",
            "password": "a long example password"
        }))
        .to_request();
    let login_response = test::call_service(&app, login).await;
    assert_eq!(login_response.status(), 200);

    let me = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header((header::AUTHORIZATION, format!("Bearer {access_token}")))
        .to_request();
    let me_response = test::call_service(&app, me).await;
    assert_eq!(me_response.status(), 200);
    let me_body: Value = test::read_body_json(me_response).await;
    assert_eq!(me_body["display_name"], "Ada Investor");

    let refresh = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .set_json(json!({ "refresh_token": refresh_token }))
        .to_request();
    let refresh_response = test::call_service(&app, refresh).await;
    assert_eq!(refresh_response.status(), 200);
    let refresh_body: Value = test::read_body_json(refresh_response).await;
    let rotated_access_token = refresh_body["access_token"]
        .as_str()
        .expect("rotated access token");

    let replay = test::TestRequest::post()
        .uri("/api/v1/auth/refresh")
        .set_json(json!({ "refresh_token": refresh_token }))
        .to_request();
    assert_eq!(test::call_service(&app, replay).await.status(), 401);

    let logout = test::TestRequest::post()
        .uri("/api/v1/auth/logout")
        .insert_header((
            header::AUTHORIZATION,
            format!("Bearer {rotated_access_token}"),
        ))
        .to_request();
    assert_eq!(test::call_service(&app, logout).await.status(), 204);

    let revoked_me = test::TestRequest::get()
        .uri("/api/v1/auth/me")
        .insert_header((
            header::AUTHORIZATION,
            format!("Bearer {rotated_access_token}"),
        ))
        .to_request();
    assert_eq!(test::call_service(&app, revoked_me).await.status(), 401);
}

#[sqlx::test(migrations = "./migrations")]
async fn rejects_weak_registration_input(pool: PgPool) {
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(AppState { database: pool }))
            .configure(configure_api),
    )
    .await;

    let request = test::TestRequest::post()
        .uri("/api/v1/auth/register")
        .set_json(json!({
            "display_name": "",
            "email": "not-an-email",
            "password": "short"
        }))
        .to_request();
    assert_eq!(test::call_service(&app, request).await.status(), 400);
}
