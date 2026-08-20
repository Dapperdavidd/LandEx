use crate::investment::{LocationFeatureCounts, PropertyScoreInputs};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone)]
pub struct PropertyRepository {
    pool: PgPool,
}

impl PropertyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn search(
        &self,
        filters: &PropertySearchFilters,
    ) -> Result<PropertyPage, sqlx::Error> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
            SELECT
                p.id,
                p.property_type,
                p.address_line,
                p.postal_code,
                p.latitude,
                p.longitude,
                p.bedrooms,
                p.bathrooms,
                p.area_sqm,
                p.year_built,
                loc.id AS location_id,
                loc.name AS location_name,
                loc.country_code,
                l.id AS listing_id,
                l.listing_type,
                l.status AS listing_status,
                l.price,
                l.currency,
                l.price_period,
                l.source_url,
                l.last_seen_at,
                COUNT(*) OVER() AS total_count
            FROM listings l
            INNER JOIN properties p ON p.id = l.property_id
            INNER JOIN locations loc ON loc.id = p.location_id
            WHERE l.status = 'active'
            "#,
        );

        apply_filters(&mut query, filters);
        query.push(" ORDER BY l.last_seen_at DESC, l.id DESC LIMIT ");
        query.push_bind(filters.limit);
        query.push(" OFFSET ");
        query.push_bind(filters.offset);

        let rows = query
            .build_query_as::<PropertyListRow>()
            .fetch_all(&self.pool)
            .await?;
        let total = rows.first().map_or(0, |row| row.total_count);
        let items = rows.into_iter().map(PropertyListItem::from).collect();

        Ok(PropertyPage {
            items,
            total,
            limit: filters.limit,
            offset: filters.offset,
        })
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<PropertyListItem>, sqlx::Error> {
        let row = sqlx::query_as::<_, PropertyListRow>(
            r#"
            SELECT
                p.id,
                p.property_type,
                p.address_line,
                p.postal_code,
                p.latitude,
                p.longitude,
                p.bedrooms,
                p.bathrooms,
                p.area_sqm,
                p.year_built,
                loc.id AS location_id,
                loc.name AS location_name,
                loc.country_code,
                l.id AS listing_id,
                l.listing_type,
                l.status AS listing_status,
                l.price,
                l.currency,
                l.price_period,
                l.source_url,
                l.last_seen_at,
                1::BIGINT AS total_count
            FROM properties p
            INNER JOIN locations loc ON loc.id = p.location_id
            INNER JOIN listings l ON l.property_id = p.id
            WHERE p.id = $1 AND l.status = 'active'
            ORDER BY l.last_seen_at DESC
            LIMIT 1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(PropertyListItem::from))
    }

    pub async fn history(
        &self,
        id: Uuid,
        limit: i64,
    ) -> Result<Vec<PropertyHistoryPoint>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT
                observed_on,
                asking_price,
                rental_price_monthly,
                shortlet_price_nightly,
                estimated_value,
                currency,
                days_on_market
            FROM property_observations
            WHERE property_id = $1
            ORDER BY observed_on DESC, created_at DESC
            LIMIT $2
            "#,
        )
        .bind(id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn score_inputs(&self, id: Uuid) -> Result<Option<PropertyScoreInputs>, sqlx::Error> {
        let row = sqlx::query_as::<_, PropertyScoreInputRow>(
            r#"
            SELECT
                (SELECT l.price FROM listings l WHERE l.property_id = p.id AND l.status = 'active' AND l.listing_type = 'sale' ORDER BY l.last_seen_at DESC LIMIT 1) AS sale_price,
                (SELECT o.rental_price_monthly FROM property_observations o WHERE o.property_id = p.id AND o.rental_price_monthly IS NOT NULL ORDER BY o.observed_on DESC, o.created_at DESC LIMIT 1) AS monthly_rent,
                (SELECT o.days_on_market FROM property_observations o WHERE o.property_id = p.id AND o.days_on_market IS NOT NULL ORDER BY o.observed_on DESC, o.created_at DESC LIMIT 1) AS days_on_market,
                (SELECT mo.annual_growth_percent FROM markets m JOIN market_observations mo ON mo.market_id = m.id WHERE m.location_id = p.location_id AND (m.property_type = p.property_type OR m.property_type IS NULL) AND mo.annual_growth_percent IS NOT NULL ORDER BY (m.property_type IS NOT NULL) DESC, mo.observed_on DESC, mo.created_at DESC LIMIT 1) AS annual_growth_percent,
                COUNT(pnf.feature_id)::BIGINT AS feature_count,
                COUNT(pnf.feature_id) FILTER (WHERE nf.category = 'transport')::BIGINT AS transport,
                COUNT(pnf.feature_id) FILTER (WHERE nf.category = 'education')::BIGINT AS education,
                COUNT(pnf.feature_id) FILTER (WHERE nf.category = 'healthcare')::BIGINT AS healthcare,
                COUNT(pnf.feature_id) FILTER (WHERE nf.category = 'commerce')::BIGINT AS commerce
            FROM properties p
            LEFT JOIN property_nearby_features pnf ON pnf.property_id = p.id AND pnf.expires_at > NOW()
            LEFT JOIN nearby_features nf ON nf.id = pnf.feature_id
            WHERE p.id = $1
            GROUP BY p.id
            "#,
        ).bind(id).fetch_optional(&self.pool).await?;
        Ok(row.map(|row| PropertyScoreInputs {
            gross_rental_yield_percent: row.sale_price.zip(row.monthly_rent).map(
                |(price, rent)| (rent * Decimal::from(12) * Decimal::from(100) / price).round_dp(4),
            ),
            annual_growth_percent: row.annual_growth_percent,
            days_on_market: row.days_on_market.map(Decimal::from),
            location: (row.feature_count > 0).then_some(LocationFeatureCounts {
                transport: row.transport.max(0) as u16,
                education: row.education.max(0) as u16,
                healthcare: row.healthcare.max(0) as u16,
                commerce: row.commerce.max(0) as u16,
            }),
        }))
    }
}

fn apply_filters<'a>(query: &mut QueryBuilder<'a, Postgres>, filters: &'a PropertySearchFilters) {
    if let Some(country_code) = &filters.country_code {
        query.push(" AND loc.country_code = ");
        query.push_bind(country_code);
    }
    if let Some(location_id) = filters.location_id {
        query.push(" AND loc.id = ");
        query.push_bind(location_id);
    }
    if let Some(property_type) = &filters.property_type {
        query.push(" AND p.property_type = ");
        query.push_bind(property_type);
    }
    if let Some(listing_type) = &filters.listing_type {
        query.push(" AND l.listing_type = ");
        query.push_bind(listing_type);
    }
    if let Some(min_price) = filters.min_price {
        query.push(" AND l.price >= ");
        query.push_bind(min_price);
    }
    if let Some(max_price) = filters.max_price {
        query.push(" AND l.price <= ");
        query.push_bind(max_price);
    }
    if let Some(currency) = &filters.currency {
        query.push(" AND l.currency = ");
        query.push_bind(currency);
    }
}

#[derive(Clone, Debug)]
pub struct PropertySearchFilters {
    pub country_code: Option<String>,
    pub location_id: Option<Uuid>,
    pub property_type: Option<String>,
    pub listing_type: Option<String>,
    pub min_price: Option<Decimal>,
    pub max_price: Option<Decimal>,
    pub currency: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, FromRow)]
struct PropertyListRow {
    id: Uuid,
    property_type: String,
    address_line: Option<String>,
    postal_code: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    bedrooms: Option<Decimal>,
    bathrooms: Option<Decimal>,
    area_sqm: Option<Decimal>,
    year_built: Option<i16>,
    location_id: Uuid,
    location_name: String,
    country_code: String,
    listing_id: Uuid,
    listing_type: String,
    listing_status: String,
    price: Decimal,
    currency: String,
    price_period: String,
    source_url: Option<String>,
    last_seen_at: DateTime<Utc>,
    total_count: i64,
}

#[derive(Debug, FromRow)]
struct PropertyScoreInputRow {
    sale_price: Option<Decimal>,
    monthly_rent: Option<Decimal>,
    days_on_market: Option<i32>,
    annual_growth_percent: Option<Decimal>,
    feature_count: i64,
    transport: i64,
    education: i64,
    healthcare: i64,
    commerce: i64,
}

#[derive(Debug, Serialize)]
pub struct PropertyListItem {
    pub id: Uuid,
    pub property_type: String,
    pub address_line: Option<String>,
    pub postal_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub bedrooms: Option<Decimal>,
    pub bathrooms: Option<Decimal>,
    pub area_sqm: Option<Decimal>,
    pub year_built: Option<i16>,
    pub location_id: Uuid,
    pub location_name: String,
    pub country_code: String,
    pub listing_id: Uuid,
    pub listing_type: String,
    pub listing_status: String,
    pub price: Decimal,
    pub currency: String,
    pub price_period: String,
    pub source_url: Option<String>,
    pub last_seen_at: DateTime<Utc>,
}

impl From<PropertyListRow> for PropertyListItem {
    fn from(row: PropertyListRow) -> Self {
        Self {
            id: row.id,
            property_type: row.property_type,
            address_line: row.address_line,
            postal_code: row.postal_code,
            latitude: row.latitude,
            longitude: row.longitude,
            bedrooms: row.bedrooms,
            bathrooms: row.bathrooms,
            area_sqm: row.area_sqm,
            year_built: row.year_built,
            location_id: row.location_id,
            location_name: row.location_name,
            country_code: row.country_code,
            listing_id: row.listing_id,
            listing_type: row.listing_type,
            listing_status: row.listing_status,
            price: row.price,
            currency: row.currency,
            price_period: row.price_period,
            source_url: row.source_url,
            last_seen_at: row.last_seen_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PropertyPage {
    pub items: Vec<PropertyListItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct PropertyHistoryPoint {
    pub observed_on: NaiveDate,
    pub asking_price: Option<Decimal>,
    pub rental_price_monthly: Option<Decimal>,
    pub shortlet_price_nightly: Option<Decimal>,
    pub estimated_value: Option<Decimal>,
    pub currency: String,
    pub days_on_market: Option<i32>,
}
