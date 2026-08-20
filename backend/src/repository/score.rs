use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

pub const METHODOLOGY_VERSION: &str = "landex-score-v1";

#[derive(Clone)]
pub struct ScoreRepository {
    pool: PgPool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ScoreObservation {
    pub methodology_version: String,
    pub observed_on: NaiveDate,
    #[serde(with = "rust_decimal::serde::str_option")]
    pub overall_score: Option<Decimal>,
    pub components: Value,
    pub unavailable_components: Value,
}

impl ScoreRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn property_ids(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        sqlx::query_scalar("SELECT id FROM properties ORDER BY id")
            .fetch_all(&self.pool)
            .await
    }
    pub async fn market_ids(&self) -> Result<Vec<Uuid>, sqlx::Error> {
        sqlx::query_scalar("SELECT id FROM markets ORDER BY id")
            .fetch_all(&self.pool)
            .await
    }
    pub async fn upsert(
        &self,
        property_id: Option<Uuid>,
        market_id: Option<Uuid>,
        overall_score: Option<Decimal>,
        components: Value,
        unavailable: Value,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(r#"INSERT INTO score_observations(property_id,market_id,methodology_version,observed_on,overall_score,components,unavailable_components)
            VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(property_id,market_id,methodology_version,observed_on) DO UPDATE SET overall_score=EXCLUDED.overall_score,components=EXCLUDED.components,unavailable_components=EXCLUDED.unavailable_components,created_at=NOW()"#)
            .bind(property_id).bind(market_id).bind(METHODOLOGY_VERSION).bind(Utc::now().date_naive()).bind(overall_score).bind(components).bind(unavailable).execute(&self.pool).await?;
        Ok(())
    }
    pub async fn property_history(
        &self,
        id: Uuid,
        limit: i64,
    ) -> Result<Vec<ScoreObservation>, sqlx::Error> {
        self.history("property_id", id, limit).await
    }
    pub async fn market_history(
        &self,
        id: Uuid,
        limit: i64,
    ) -> Result<Vec<ScoreObservation>, sqlx::Error> {
        self.history("market_id", id, limit).await
    }
    async fn history(
        &self,
        column: &str,
        id: Uuid,
        limit: i64,
    ) -> Result<Vec<ScoreObservation>, sqlx::Error> {
        let query = format!(
            "SELECT methodology_version,observed_on,overall_score,components,unavailable_components FROM score_observations WHERE {column}=$1 ORDER BY observed_on DESC,created_at DESC LIMIT $2"
        );
        sqlx::query_as(&query)
            .bind(id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
    }
}
