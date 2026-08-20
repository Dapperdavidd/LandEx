use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Clone)]
pub struct ProviderRepository {
    pool: PgPool,
}

#[derive(Debug, FromRow)]
struct ProviderStatusRow {
    id: Uuid,
    slug: String,
    name: String,
    enabled: bool,
    attempts_24h: i64,
    attempts_32d: i64,
    successful_attempts_32d: i64,
    failed_attempts_32d: i64,
    reserved_attempts_32d: i64,
    last_attempt_at: Option<DateTime<Utc>>,
    last_attempt_outcome: Option<String>,
    last_data_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct ProviderStatus {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub enabled: bool,
    pub health: String,
    pub stale: bool,
    pub attempts_24h: i64,
    pub attempts_32d: i64,
    pub successful_attempts_32d: i64,
    pub failed_attempts_32d: i64,
    pub reserved_attempts_32d: i64,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub last_attempt_outcome: Option<String>,
    pub last_data_at: Option<DateTime<Utc>>,
}

impl ProviderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn statuses(&self) -> Result<Vec<ProviderStatus>, sqlx::Error> {
        let rows = sqlx::query_as::<_, ProviderStatusRow>(
            r#"
            SELECT p.id, p.slug, p.name, p.enabled,
                COUNT(DISTINCT a.id) FILTER (WHERE a.requested_at >= NOW() - INTERVAL '1 day') AS attempts_24h,
                COUNT(DISTINCT a.id) FILTER (WHERE a.requested_at >= NOW() - INTERVAL '32 days') AS attempts_32d,
                COUNT(DISTINCT a.id) FILTER (WHERE a.requested_at >= NOW() - INTERVAL '32 days' AND a.outcome = 'succeeded') AS successful_attempts_32d,
                COUNT(DISTINCT a.id) FILTER (WHERE a.requested_at >= NOW() - INTERVAL '32 days' AND a.outcome = 'failed') AS failed_attempts_32d,
                COUNT(DISTINCT a.id) FILTER (WHERE a.requested_at >= NOW() - INTERVAL '32 days' AND a.outcome = 'reserved') AS reserved_attempts_32d,
                latest.requested_at AS last_attempt_at,
                latest.outcome AS last_attempt_outcome,
                GREATEST(MAX(pl.last_seen_at), MAX(pp.last_seen_at), MAX(l.last_seen_at)) AS last_data_at
            FROM providers p
            LEFT JOIN provider_request_attempts a ON a.provider_id = p.id
            LEFT JOIN provider_locations pl ON pl.provider_id = p.id
            LEFT JOIN provider_properties pp ON pp.provider_id = p.id
            LEFT JOIN listings l ON l.provider_id = p.id
            LEFT JOIN LATERAL (
                SELECT requested_at, outcome FROM provider_request_attempts
                WHERE provider_id = p.id ORDER BY requested_at DESC LIMIT 1
            ) latest ON TRUE
            GROUP BY p.id, latest.requested_at, latest.outcome
            ORDER BY p.slug
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        let now = Utc::now();
        Ok(rows
            .into_iter()
            .map(|row| {
                let stale = row
                    .last_data_at
                    .is_none_or(|value| now.signed_duration_since(value).num_days() >= 7);
                let health = if !row.enabled {
                    "disabled"
                } else if row.last_attempt_outcome.as_deref() == Some("failed") {
                    "degraded"
                } else if stale {
                    "stale"
                } else {
                    "healthy"
                };
                ProviderStatus {
                    id: row.id,
                    slug: row.slug,
                    name: row.name,
                    enabled: row.enabled,
                    health: health.to_owned(),
                    stale,
                    attempts_24h: row.attempts_24h,
                    attempts_32d: row.attempts_32d,
                    successful_attempts_32d: row.successful_attempts_32d,
                    failed_attempts_32d: row.failed_attempts_32d,
                    reserved_attempts_32d: row.reserved_attempts_32d,
                    last_attempt_at: row.last_attempt_at,
                    last_attempt_outcome: row.last_attempt_outcome,
                    last_data_at: row.last_data_at,
                }
            })
            .collect())
    }
}
