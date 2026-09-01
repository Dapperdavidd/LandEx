use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone)]
pub struct InstrumentRepository {
    pool: PgPool,
}

impl InstrumentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn search(&self, filters: &InstrumentFilters) -> Result<InstrumentPage, sqlx::Error> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
            SELECT i.id, i.slug, i.name, i.instrument_kind, i.status,
                   i.country_code, i.currency, i.symbol, i.exchange,
                   i.location_id, i.property_id, i.source_url, i.valuation_method,
                   i.liquidity_class, i.real_money_enabled, i.metadata,
                   latest.observed_on, latest.value, latest.annual_change_percent,
                   latest.income_yield_percent, COUNT(*) OVER() AS total_count
            FROM investment_instruments i
            LEFT JOIN LATERAL (
                SELECT observed_on, value, annual_change_percent, income_yield_percent
                FROM instrument_observations
                WHERE instrument_id = i.id
                ORDER BY observed_on DESC LIMIT 1
            ) latest ON TRUE
            WHERE TRUE
        "#,
        );
        if let Some(kind) = &filters.kind {
            query.push(" AND i.instrument_kind = ").push_bind(kind);
        }
        if let Some(status) = &filters.status {
            query.push(" AND i.status = ").push_bind(status);
        }
        if let Some(country) = &filters.country_code {
            query.push(" AND i.country_code = ").push_bind(country);
        }
        query
            .push(" ORDER BY i.name, i.id LIMIT ")
            .push_bind(filters.limit);
        query.push(" OFFSET ").push_bind(filters.offset);
        let rows = query
            .build_query_as::<InstrumentRow>()
            .fetch_all(&self.pool)
            .await?;
        let total = rows.first().map_or(0, |row| row.total_count);
        Ok(InstrumentPage {
            items: rows.into_iter().map(InstrumentSummary::from).collect(),
            total,
            limit: filters.limit,
            offset: filters.offset,
        })
    }

    pub async fn find(
        &self,
        id: Uuid,
        history_limit: i64,
    ) -> Result<Option<InstrumentDetail>, sqlx::Error> {
        let row = sqlx::query_as::<_, InstrumentIdentity>(
            r#"
            SELECT id, slug, name, instrument_kind, status, country_code, currency,
                   symbol, exchange, location_id, property_id, source_url,
                   valuation_method, liquidity_class, real_money_enabled, metadata
            FROM investment_instruments WHERE id = $1
        "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let history = sqlx::query_as::<_, InstrumentObservation>(
            r#"
            SELECT observed_on, value, currency, annual_change_percent,
                   income_yield_percent, source_url, methodology, metadata
            FROM instrument_observations WHERE instrument_id = $1
            ORDER BY observed_on DESC LIMIT $2
        "#,
        )
        .bind(id)
        .bind(history_limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(Some(InstrumentDetail {
            instrument: row,
            history,
        }))
    }

    pub async fn coverage(&self) -> Result<Vec<CountryCoverage>, sqlx::Error> {
        sqlx::query_as::<_, CountryCoverage>(
            r#"
            SELECT country_code, country_name, coverage_depth, has_market_data,
                   has_historical_data, has_active_listings, has_investible_offerings,
                   provider_slugs, methodology, latest_observation_on, updated_at
            FROM country_coverage ORDER BY country_name
        "#,
        )
        .fetch_all(&self.pool)
        .await
    }
}

#[derive(Debug)]
pub struct InstrumentFilters {
    pub kind: Option<String>,
    pub status: Option<String>,
    pub country_code: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, FromRow)]
struct InstrumentRow {
    id: Uuid,
    slug: String,
    name: String,
    instrument_kind: String,
    status: String,
    country_code: String,
    currency: String,
    symbol: Option<String>,
    exchange: Option<String>,
    location_id: Option<Uuid>,
    property_id: Option<Uuid>,
    source_url: Option<String>,
    valuation_method: String,
    liquidity_class: String,
    real_money_enabled: bool,
    metadata: Value,
    observed_on: Option<NaiveDate>,
    value: Option<Decimal>,
    annual_change_percent: Option<Decimal>,
    income_yield_percent: Option<Decimal>,
    total_count: i64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct InstrumentIdentity {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub instrument_kind: String,
    pub status: String,
    pub country_code: String,
    pub currency: String,
    pub symbol: Option<String>,
    pub exchange: Option<String>,
    pub location_id: Option<Uuid>,
    pub property_id: Option<Uuid>,
    pub source_url: Option<String>,
    pub valuation_method: String,
    pub liquidity_class: String,
    pub real_money_enabled: bool,
    pub metadata: Value,
}

#[derive(Debug, Serialize)]
pub struct InstrumentSummary {
    #[serde(flatten)]
    pub instrument: InstrumentIdentity,
    pub observed_on: Option<NaiveDate>,
    pub value: Option<Decimal>,
    pub annual_change_percent: Option<Decimal>,
    pub income_yield_percent: Option<Decimal>,
}

impl From<InstrumentRow> for InstrumentSummary {
    fn from(r: InstrumentRow) -> Self {
        Self {
            instrument: InstrumentIdentity {
                id: r.id,
                slug: r.slug,
                name: r.name,
                instrument_kind: r.instrument_kind,
                status: r.status,
                country_code: r.country_code,
                currency: r.currency,
                symbol: r.symbol,
                exchange: r.exchange,
                location_id: r.location_id,
                property_id: r.property_id,
                source_url: r.source_url,
                valuation_method: r.valuation_method,
                liquidity_class: r.liquidity_class,
                real_money_enabled: r.real_money_enabled,
                metadata: r.metadata,
            },
            observed_on: r.observed_on,
            value: r.value,
            annual_change_percent: r.annual_change_percent,
            income_yield_percent: r.income_yield_percent,
        }
    }
}

#[derive(Debug, FromRow, Serialize)]
pub struct InstrumentObservation {
    pub observed_on: NaiveDate,
    pub value: Decimal,
    pub currency: String,
    pub annual_change_percent: Option<Decimal>,
    pub income_yield_percent: Option<Decimal>,
    pub source_url: Option<String>,
    pub methodology: String,
    pub metadata: Value,
}

#[derive(Debug, Serialize)]
pub struct InstrumentDetail {
    #[serde(flatten)]
    pub instrument: InstrumentIdentity,
    pub history: Vec<InstrumentObservation>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct CountryCoverage {
    pub country_code: String,
    pub country_name: String,
    pub coverage_depth: String,
    pub has_market_data: bool,
    pub has_historical_data: bool,
    pub has_active_listings: bool,
    pub has_investible_offerings: bool,
    pub provider_slugs: Value,
    pub methodology: Option<String>,
    pub latest_observation_on: Option<NaiveDate>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct InstrumentPage {
    pub items: Vec<InstrumentSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}
