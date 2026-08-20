use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct SavedSearchRepository {
    pool: PgPool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SavedSearch {
    pub id: Uuid,
    pub name: String,
    pub criteria: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SavedSearchRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, user_id: Uuid) -> Result<Vec<SavedSearch>, sqlx::Error> {
        sqlx::query_as(
            "SELECT id, name, criteria, created_at, updated_at FROM saved_searches WHERE user_id = $1 ORDER BY updated_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn create(
        &self,
        user_id: Uuid,
        name: &str,
        criteria: Value,
    ) -> Result<SavedSearch, sqlx::Error> {
        sqlx::query_as(
            "INSERT INTO saved_searches (user_id, name, criteria) VALUES ($1,$2,$3) RETURNING id, name, criteria, created_at, updated_at",
        )
        .bind(user_id)
        .bind(name)
        .bind(criteria)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get(&self, user_id: Uuid, id: Uuid) -> Result<Option<SavedSearch>, sqlx::Error> {
        sqlx::query_as(
            "SELECT id, name, criteria, created_at, updated_at FROM saved_searches WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn update(
        &self,
        user_id: Uuid,
        id: Uuid,
        name: &str,
        criteria: Value,
    ) -> Result<Option<SavedSearch>, sqlx::Error> {
        sqlx::query_as(
            "UPDATE saved_searches SET name = $3, criteria = $4, updated_at = NOW() WHERE id = $1 AND user_id = $2 RETURNING id, name, criteria, created_at, updated_at",
        )
        .bind(id)
        .bind(user_id)
        .bind(name)
        .bind(criteria)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete(&self, user_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
        Ok(
            sqlx::query("DELETE FROM saved_searches WHERE id = $1 AND user_id = $2")
                .bind(id)
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected()
                == 1,
        )
    }
}
