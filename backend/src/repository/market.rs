use crate::investment::PropertyScoreInputs;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone)]
pub struct MarketRepository {
    pool: PgPool,
}

impl MarketRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn search(&self, filters: &MarketSearchFilters) -> Result<MarketPage, sqlx::Error> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
            SELECT
                m.id,
                m.name,
                m.property_type,
                loc.id AS location_id,
                loc.name AS location_name,
                loc.kind AS location_kind,
                loc.country_code,
                loc.latitude,
                loc.longitude,
                latest.observed_on,
                latest.currency,
                latest.median_sale_price,
                latest.median_rent_monthly,
                latest.gross_yield_percent,
                latest.annual_growth_percent,
                latest.active_inventory,
                latest.days_on_market,
                COUNT(*) OVER() AS total_count
            FROM markets m
            INNER JOIN locations loc ON loc.id = m.location_id
            LEFT JOIN LATERAL (
                SELECT
                    observed_on,
                    currency,
                    median_sale_price,
                    median_rent_monthly,
                    gross_yield_percent,
                    annual_growth_percent,
                    active_inventory,
                    days_on_market
                FROM market_observations
                WHERE market_id = m.id
                ORDER BY observed_on DESC, created_at DESC
                LIMIT 1
            ) latest ON TRUE
            WHERE TRUE
            "#,
        );

        if let Some(country_code) = &filters.country_code {
            query.push(" AND loc.country_code = ");
            query.push_bind(country_code);
        }
        if let Some(location_id) = filters.location_id {
            query.push(" AND loc.id = ");
            query.push_bind(location_id);
        }
        if let Some(property_type) = &filters.property_type {
            query.push(" AND m.property_type = ");
            query.push_bind(property_type);
        }
        if let Some(currency) = &filters.currency {
            query.push(" AND latest.currency = ");
            query.push_bind(currency);
        }

        query.push(" ORDER BY m.name ASC, m.id ASC LIMIT ");
        query.push_bind(filters.limit);
        query.push(" OFFSET ");
        query.push_bind(filters.offset);

        let rows = query
            .build_query_as::<MarketRow>()
            .fetch_all(&self.pool)
            .await?;
        let total = rows.first().map_or(0, |row| row.total_count);

        Ok(MarketPage {
            items: rows.into_iter().map(MarketSummary::from).collect(),
            total,
            limit: filters.limit,
            offset: filters.offset,
        })
    }

    pub async fn find_by_id(
        &self,
        id: Uuid,
        history_limit: i64,
    ) -> Result<Option<MarketDetail>, sqlx::Error> {
        let market = sqlx::query_as::<_, MarketIdentityRow>(
            r#"
            SELECT
                m.id,
                m.name,
                m.property_type,
                loc.id AS location_id,
                loc.name AS location_name,
                loc.kind AS location_kind,
                loc.country_code
            FROM markets m
            INNER JOIN locations loc ON loc.id = m.location_id
            WHERE m.id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(market) = market else {
            return Ok(None);
        };

        let history = sqlx::query_as::<_, MarketMetric>(
            r#"
            SELECT
                observed_on,
                currency,
                median_sale_price,
                median_rent_monthly,
                gross_yield_percent,
                annual_growth_percent,
                active_inventory,
                days_on_market
            FROM market_observations
            WHERE market_id = $1
            ORDER BY observed_on DESC, created_at DESC
            LIMIT $2
            "#,
        )
        .bind(id)
        .bind(history_limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(Some(MarketDetail {
            id: market.id,
            name: market.name,
            property_type: market.property_type,
            location_id: market.location_id,
            location_name: market.location_name,
            location_kind: market.location_kind,
            country_code: market.country_code,
            history,
        }))
    }

    pub async fn score_inputs(&self, id: Uuid) -> Result<Option<PropertyScoreInputs>, sqlx::Error> {
        let metric = sqlx::query_as::<_, MarketScoreInputRow>(
            r#"
            SELECT mo.gross_yield_percent, mo.annual_growth_percent, mo.days_on_market
            FROM markets m LEFT JOIN LATERAL (
                SELECT gross_yield_percent, annual_growth_percent, days_on_market
                FROM market_observations WHERE market_id=m.id
                ORDER BY observed_on DESC, created_at DESC LIMIT 1
            ) mo ON TRUE WHERE m.id=$1
        "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(metric.map(|row| PropertyScoreInputs {
            gross_rental_yield_percent: row.gross_yield_percent,
            annual_growth_percent: row.annual_growth_percent,
            days_on_market: row.days_on_market,
            location: None,
        }))
    }
}

#[derive(Debug, FromRow)]
struct MarketScoreInputRow {
    gross_yield_percent: Option<Decimal>,
    annual_growth_percent: Option<Decimal>,
    days_on_market: Option<Decimal>,
}

#[derive(Clone, Debug)]
pub struct MarketSearchFilters {
    pub country_code: Option<String>,
    pub location_id: Option<Uuid>,
    pub property_type: Option<String>,
    pub currency: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, FromRow)]
struct MarketIdentityRow {
    id: Uuid,
    name: String,
    property_type: Option<String>,
    location_id: Uuid,
    location_name: String,
    location_kind: String,
    country_code: String,
}

#[derive(Debug, FromRow)]
struct MarketRow {
    id: Uuid,
    name: String,
    property_type: Option<String>,
    location_id: Uuid,
    location_name: String,
    location_kind: String,
    country_code: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
    observed_on: Option<NaiveDate>,
    currency: Option<String>,
    median_sale_price: Option<Decimal>,
    median_rent_monthly: Option<Decimal>,
    gross_yield_percent: Option<Decimal>,
    annual_growth_percent: Option<Decimal>,
    active_inventory: Option<i32>,
    days_on_market: Option<Decimal>,
    total_count: i64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct MarketMetric {
    pub observed_on: NaiveDate,
    pub currency: String,
    pub median_sale_price: Option<Decimal>,
    pub median_rent_monthly: Option<Decimal>,
    pub gross_yield_percent: Option<Decimal>,
    pub annual_growth_percent: Option<Decimal>,
    pub active_inventory: Option<i32>,
    pub days_on_market: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct LatestMarketMetric {
    pub observed_on: Option<NaiveDate>,
    pub currency: Option<String>,
    pub median_sale_price: Option<Decimal>,
    pub median_rent_monthly: Option<Decimal>,
    pub gross_yield_percent: Option<Decimal>,
    pub annual_growth_percent: Option<Decimal>,
    pub active_inventory: Option<i32>,
    pub days_on_market: Option<Decimal>,
}

#[derive(Debug, Serialize)]
pub struct MarketSummary {
    pub id: Uuid,
    pub name: String,
    pub property_type: Option<String>,
    pub location_id: Uuid,
    pub location_name: String,
    pub location_kind: String,
    pub country_code: String,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub latest: LatestMarketMetric,
}

impl From<MarketRow> for MarketSummary {
    fn from(row: MarketRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            property_type: row.property_type,
            location_id: row.location_id,
            location_name: row.location_name,
            location_kind: row.location_kind,
            country_code: row.country_code,
            latitude: row.latitude,
            longitude: row.longitude,
            latest: LatestMarketMetric {
                observed_on: row.observed_on,
                currency: row.currency,
                median_sale_price: row.median_sale_price,
                median_rent_monthly: row.median_rent_monthly,
                gross_yield_percent: row.gross_yield_percent,
                annual_growth_percent: row.annual_growth_percent,
                active_inventory: row.active_inventory,
                days_on_market: row.days_on_market,
            },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MarketPage {
    pub items: Vec<MarketSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct MarketDetail {
    pub id: Uuid,
    pub name: String,
    pub property_type: Option<String>,
    pub location_id: Uuid,
    pub location_name: String,
    pub location_kind: String,
    pub country_code: String,
    pub history: Vec<MarketMetric>,
}
