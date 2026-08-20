use chrono::{DateTime, Utc};
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
}
