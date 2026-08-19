use chrono::NaiveDate;
use sqlx::PgPool;

#[derive(Clone)]
pub struct MarketAggregationService {
    pool: PgPool,
}

impl MarketAggregationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn refresh(
        &self,
        observed_on: NaiveDate,
    ) -> Result<MarketAggregationReport, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;

        let markets_affected = sqlx::query(
            r#"
            INSERT INTO markets (location_id, property_type, name)
            SELECT DISTINCT
                p.location_id,
                p.property_type,
                loc.name || ' ' || INITCAP(REPLACE(p.property_type, '_', ' '))
            FROM properties p
            INNER JOIN locations loc ON loc.id = p.location_id
            INNER JOIN listings l ON l.property_id = p.id
            WHERE l.status = 'active'
            ON CONFLICT (location_id, property_type) DO UPDATE SET
                name = EXCLUDED.name,
                updated_at = NOW()
            "#,
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();

        let observations_upserted = sqlx::query(
            r#"
            WITH currency_aggregates AS (
                SELECT
                    p.location_id,
                    p.property_type,
                    l.currency,
                    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY l.price)
                        FILTER (WHERE l.listing_type = 'sale')::NUMERIC AS median_sale_price,
                    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY l.price)
                        FILTER (WHERE l.listing_type = 'rent')::NUMERIC AS median_rent_monthly,
                    COUNT(*)::INTEGER AS active_inventory,
                    AVG(
                        GREATEST(
                            0,
                            EXTRACT(EPOCH FROM (l.last_seen_at - l.listed_at)) / 86400
                        )
                    )::NUMERIC AS days_on_market,
                    ROW_NUMBER() OVER (
                        PARTITION BY p.location_id, p.property_type
                        ORDER BY COUNT(*) DESC, l.currency ASC
                    ) AS currency_rank
                FROM properties p
                INNER JOIN listings l ON l.property_id = p.id
                WHERE l.status = 'active'
                GROUP BY p.location_id, p.property_type, l.currency
            ), selected_aggregates AS (
                SELECT * FROM currency_aggregates WHERE currency_rank = 1
            )
            INSERT INTO market_observations (
                market_id,
                provider_id,
                observed_on,
                currency,
                median_sale_price,
                median_rent_monthly,
                gross_yield_percent,
                active_inventory,
                days_on_market,
                metadata
            )
            SELECT
                m.id,
                NULL,
                $1,
                aggregate.currency,
                aggregate.median_sale_price,
                aggregate.median_rent_monthly,
                CASE
                    WHEN aggregate.median_sale_price > 0
                        AND aggregate.median_rent_monthly IS NOT NULL
                    THEN (
                        aggregate.median_rent_monthly * 12
                        / aggregate.median_sale_price * 100
                    )
                    ELSE NULL
                END,
                aggregate.active_inventory,
                aggregate.days_on_market,
                jsonb_build_object('calculation', 'normalized_active_listings')
            FROM selected_aggregates aggregate
            INNER JOIN markets m
                ON m.location_id = aggregate.location_id
                AND m.property_type = aggregate.property_type
            ON CONFLICT (market_id, provider_id, observed_on) DO UPDATE SET
                currency = EXCLUDED.currency,
                median_sale_price = EXCLUDED.median_sale_price,
                median_rent_monthly = EXCLUDED.median_rent_monthly,
                gross_yield_percent = EXCLUDED.gross_yield_percent,
                active_inventory = EXCLUDED.active_inventory,
                days_on_market = EXCLUDED.days_on_market,
                metadata = EXCLUDED.metadata
            "#,
        )
        .bind(observed_on)
        .execute(&mut *transaction)
        .await?
        .rows_affected();

        transaction.commit().await?;

        Ok(MarketAggregationReport {
            markets_affected,
            observations_upserted,
            observed_on,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketAggregationReport {
    pub markets_affected: u64,
    pub observations_upserted: u64,
    pub observed_on: NaiveDate,
}
