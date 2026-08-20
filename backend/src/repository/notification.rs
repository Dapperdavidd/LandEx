use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct NotificationRepository {
    pool: PgPool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AlertRule {
    pub id: Uuid,
    pub watchlist_item_id: Uuid,
    pub alert_type: String,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub threshold: Option<Decimal>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl NotificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn list(&self, user_id: Uuid, limit: i64) -> Result<Vec<Notification>, sqlx::Error> {
        sqlx::query_as("SELECT id, notification_type, title, body, read_at, created_at FROM notifications WHERE user_id=$1 ORDER BY created_at DESC, id DESC LIMIT $2")
            .bind(user_id).bind(limit).fetch_all(&self.pool).await
    }
    pub async fn mark_read(
        &self,
        user_id: Uuid,
        id: Uuid,
    ) -> Result<Option<Notification>, sqlx::Error> {
        sqlx::query_as("UPDATE notifications SET read_at=COALESCE(read_at,NOW()) WHERE id=$1 AND user_id=$2 RETURNING id, notification_type, title, body, read_at, created_at")
            .bind(id).bind(user_id).fetch_optional(&self.pool).await
    }

    pub async fn list_rules(&self, user_id: Uuid) -> Result<Vec<AlertRule>, sqlx::Error> {
        sqlx::query_as("SELECT id, watchlist_item_id, alert_type, threshold, enabled, created_at, updated_at FROM alert_rules WHERE user_id=$1 ORDER BY created_at DESC, id DESC")
            .bind(user_id).fetch_all(&self.pool).await
    }

    pub async fn create_rule(
        &self,
        user_id: Uuid,
        item_id: Uuid,
        alert_type: &str,
        threshold: Option<Decimal>,
    ) -> Result<Option<AlertRule>, sqlx::Error> {
        sqlx::query_as(r#"INSERT INTO alert_rules (user_id, watchlist_item_id, alert_type, threshold)
            SELECT $1, wi.id, $3, $4 FROM watchlist_items wi JOIN watchlists w ON w.id=wi.watchlist_id
            WHERE wi.id=$2 AND w.user_id=$1
            RETURNING id, watchlist_item_id, alert_type, threshold, enabled, created_at, updated_at"#)
            .bind(user_id).bind(item_id).bind(alert_type).bind(threshold).fetch_optional(&self.pool).await
    }

    pub async fn set_rule_enabled(
        &self,
        user_id: Uuid,
        id: Uuid,
        enabled: bool,
    ) -> Result<Option<AlertRule>, sqlx::Error> {
        sqlx::query_as("UPDATE alert_rules SET enabled=$3, updated_at=NOW() WHERE id=$1 AND user_id=$2 RETURNING id, watchlist_item_id, alert_type, threshold, enabled, created_at, updated_at")
            .bind(id).bind(user_id).bind(enabled).fetch_optional(&self.pool).await
    }

    pub async fn delete_rule(&self, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
        Ok(
            sqlx::query("DELETE FROM alert_rules WHERE id=$1 AND user_id=$2")
                .bind(id)
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected()
                == 1,
        )
    }
}
