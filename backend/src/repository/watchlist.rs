use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct WatchlistRepository {
    pool: PgPool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WatchlistSummary {
    pub id: Uuid,
    pub name: String,
    pub item_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WatchlistItem {
    pub id: Uuid,
    pub property_id: Option<Uuid>,
    pub market_id: Option<Uuid>,
    pub location_id: Option<Uuid>,
    pub instrument_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WatchlistDetail {
    #[serde(flatten)]
    pub watchlist: WatchlistSummary,
    pub items: Vec<WatchlistItem>,
}

#[derive(Debug)]
pub enum WatchTarget {
    Property(Uuid),
    Market(Uuid),
    Location(Uuid),
    Instrument(Uuid),
}

impl WatchlistRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(&self, user_id: Uuid) -> Result<Vec<WatchlistSummary>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT watchlists.id, watchlists.name, COUNT(watchlist_items.id) AS item_count,
                   watchlists.created_at, watchlists.updated_at
            FROM watchlists
            LEFT JOIN watchlist_items ON watchlist_items.watchlist_id = watchlists.id
            WHERE watchlists.user_id = $1
            GROUP BY watchlists.id
            ORDER BY watchlists.created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn create(&self, user_id: Uuid, name: &str) -> Result<WatchlistSummary, sqlx::Error> {
        sqlx::query_as(
            r#"
            INSERT INTO watchlists (user_id, name)
            VALUES ($1, $2)
            RETURNING id, name, 0::BIGINT AS item_count, created_at, updated_at
            "#,
        )
        .bind(user_id)
        .bind(name)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn get(
        &self,
        user_id: Uuid,
        watchlist_id: Uuid,
    ) -> Result<Option<WatchlistDetail>, sqlx::Error> {
        let Some(watchlist) = sqlx::query_as::<_, WatchlistSummary>(
            r#"
            SELECT watchlists.id, watchlists.name, COUNT(watchlist_items.id) AS item_count,
                   watchlists.created_at, watchlists.updated_at
            FROM watchlists
            LEFT JOIN watchlist_items ON watchlist_items.watchlist_id = watchlists.id
            WHERE watchlists.id = $1 AND watchlists.user_id = $2
            GROUP BY watchlists.id
            "#,
        )
        .bind(watchlist_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        let items = sqlx::query_as::<_, WatchlistItem>(
            r#"
            SELECT id, property_id, market_id, location_id, instrument_id, created_at
            FROM watchlist_items
            WHERE watchlist_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(watchlist_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(Some(WatchlistDetail { watchlist, items }))
    }

    pub async fn add_item(
        &self,
        user_id: Uuid,
        watchlist_id: Uuid,
        target: WatchTarget,
    ) -> Result<Option<WatchlistItem>, sqlx::Error> {
        let (property_id, market_id, location_id, instrument_id) = match target {
            WatchTarget::Property(id) => (Some(id), None, None, None),
            WatchTarget::Market(id) => (None, Some(id), None, None),
            WatchTarget::Location(id) => (None, None, Some(id), None),
            WatchTarget::Instrument(id) => (None, None, None, Some(id)),
        };

        sqlx::query_as(
            r#"
            INSERT INTO watchlist_items (watchlist_id, property_id, market_id, location_id, instrument_id)
            SELECT watchlists.id, $3, $4, $5, $6
            FROM watchlists
            WHERE watchlists.id = $1 AND watchlists.user_id = $2
            RETURNING id, property_id, market_id, location_id, instrument_id, created_at
            "#,
        )
        .bind(watchlist_id)
        .bind(user_id)
        .bind(property_id)
        .bind(market_id)
        .bind(location_id)
        .bind(instrument_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn remove_item(
        &self,
        user_id: Uuid,
        watchlist_id: Uuid,
        item_id: Uuid,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM watchlist_items
            USING watchlists
            WHERE watchlist_items.id = $1
              AND watchlist_items.watchlist_id = watchlists.id
              AND watchlists.id = $2
              AND watchlists.user_id = $3
            "#,
        )
        .bind(item_id)
        .bind(watchlist_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }
}
