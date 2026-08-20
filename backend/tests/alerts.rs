#![cfg(feature = "integration-tests")]

use landex_api::alerts::AlertEvaluationService;
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn initializes_then_emits_one_idempotent_price_alert(pool: PgPool) {
    let user_id: Uuid = sqlx::query_scalar("INSERT INTO users (display_name,primary_email,primary_email_normalized) VALUES ('Alert User','alert@example.com','alert@example.com') RETURNING id").fetch_one(&pool).await.unwrap();
    let provider_id: Uuid = sqlx::query_scalar(
        "INSERT INTO providers (slug,name) VALUES ('alert-fixture','Alert Fixture') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let location_id: Uuid = sqlx::query_scalar("INSERT INTO locations (kind,name,normalized_name,country_code) VALUES ('city','Lagos','lagos','NG') RETURNING id").fetch_one(&pool).await.unwrap();
    let property_id: Uuid = sqlx::query_scalar("INSERT INTO properties (location_id,property_type,latitude,longitude) VALUES ($1,'apartment',6.5,3.4) RETURNING id").bind(location_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO listings (property_id,provider_id,source_id,listing_type,status,price,currency) VALUES ($1,$2,'alert-listing','sale','active',100,'USD')").bind(property_id).bind(provider_id).execute(&pool).await.unwrap();
    let watchlist_id: Uuid = sqlx::query_scalar(
        "INSERT INTO watchlists (user_id,name) VALUES ($1,'Alerts') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let item_id: Uuid = sqlx::query_scalar(
        "INSERT INTO watchlist_items (watchlist_id,property_id) VALUES ($1,$2) RETURNING id",
    )
    .bind(watchlist_id)
    .bind(property_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO alert_rules (user_id,watchlist_item_id,alert_type,threshold) VALUES ($1,$2,'price_change',5)").bind(user_id).bind(item_id).execute(&pool).await.unwrap();

    let service = AlertEvaluationService::new(pool.clone());
    let first = service.evaluate().await.unwrap();
    assert_eq!(first.initialized, 1);
    assert_eq!(first.emitted, 0);
    sqlx::query("UPDATE listings SET price=110,last_seen_at=NOW()+INTERVAL '1 second' WHERE source_id='alert-listing'").execute(&pool).await.unwrap();
    let second = service.evaluate().await.unwrap();
    assert_eq!(second.emitted, 1);
    let third = service.evaluate().await.unwrap();
    assert_eq!(third.emitted, 0);
    let notifications: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM notifications WHERE user_id=$1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(notifications, 1);
}
