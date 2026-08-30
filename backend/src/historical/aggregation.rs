use sqlx::PgPool;

#[derive(Clone)]
pub struct HmlrMarketAggregationService {
    pool: PgPool,
}

impl HmlrMarketAggregationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn refresh(&self) -> Result<HmlrMarketAggregationReport, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let provider_id: uuid::Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO providers (slug, name)
            VALUES ('hm-land-registry', 'HM Land Registry')
            ON CONFLICT (slug) DO UPDATE SET name=EXCLUDED.name, updated_at=NOW()
            RETURNING id
            "#,
        )
        .fetch_one(&mut *transaction)
        .await?;

        let locations_created = sqlx::query(
            r#"
            INSERT INTO locations (kind, name, normalized_name, country_code)
            SELECT 'city', INITCAP(LOWER(source.town_city)), LOWER(source.town_city), 'GB'
            FROM (SELECT DISTINCT town_city FROM hmlr_price_paid_transactions) source
            WHERE NOT EXISTS (
                SELECT 1 FROM locations existing
                WHERE existing.country_code='GB'
                  AND existing.kind='city'
                  AND existing.normalized_name=LOWER(source.town_city)
            )
            ON CONFLICT DO NOTHING
            "#,
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();

        let markets_created = sqlx::query(
            r#"
            WITH source_markets AS (
                SELECT DISTINCT
                    town_city,
                    CASE
                        WHEN property_type IN ('D', 'S', 'T') THEN 'house'
                        WHEN property_type='F' THEN 'apartment'
                        ELSE 'other'
                    END AS property_type
                FROM hmlr_price_paid_transactions
            )
            INSERT INTO markets (location_id, property_type, name)
            SELECT location.id, source.property_type,
                   location.name || ' ' || INITCAP(source.property_type)
            FROM source_markets source
            CROSS JOIN LATERAL (
                SELECT id, name FROM locations
                WHERE country_code='GB' AND kind='city'
                  AND normalized_name=LOWER(source.town_city)
                ORDER BY (parent_id IS NOT NULL) DESC, created_at ASC
                LIMIT 1
            ) location
            ON CONFLICT (location_id, property_type) DO UPDATE
            SET name=EXCLUDED.name, updated_at=NOW()
            "#,
        )
        .execute(&mut *transaction)
        .await?
        .rows_affected();

        let observations_upserted = sqlx::query(
            r#"
            WITH monthly AS (
                SELECT
                    town_city,
                    CASE
                        WHEN property_type IN ('D', 'S', 'T') THEN 'house'
                        WHEN property_type='F' THEN 'apartment'
                        ELSE 'other'
                    END AS property_type,
                    DATE_TRUNC('month', transferred_on)::DATE AS observed_on,
                    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY price)::NUMERIC AS median_sale_price,
                    COUNT(*)::INTEGER AS transaction_count
                FROM hmlr_price_paid_transactions
                GROUP BY town_city, 2, DATE_TRUNC('month', transferred_on)::DATE
            ), enriched AS (
                SELECT current.*,
                    CASE WHEN previous.median_sale_price > 0 THEN
                        ((current.median_sale_price / previous.median_sale_price) - 1) * 100
                    END AS annual_growth_percent
                FROM monthly current
                LEFT JOIN monthly previous
                  ON previous.town_city=current.town_city
                 AND previous.property_type=current.property_type
                 AND previous.observed_on=(current.observed_on - INTERVAL '1 year')::DATE
            )
            INSERT INTO market_observations (
                market_id, provider_id, observed_on, currency,
                median_sale_price, annual_growth_percent, metadata
            )
            SELECT market.id, $1, source.observed_on, 'GBP',
                   source.median_sale_price, source.annual_growth_percent,
                   jsonb_build_object(
                       'calculation', 'monthly_median_registered_transactions',
                       'transaction_count', source.transaction_count,
                       'source', 'HM Land Registry Price Paid Data'
                   )
            FROM enriched source
            CROSS JOIN LATERAL (
                SELECT id FROM locations
                WHERE country_code='GB' AND kind='city'
                  AND normalized_name=LOWER(source.town_city)
                ORDER BY (parent_id IS NOT NULL) DESC, created_at ASC
                LIMIT 1
            ) location
            INNER JOIN markets market
              ON market.location_id=location.id
             AND market.property_type=source.property_type
            ON CONFLICT (market_id, provider_id, observed_on) DO UPDATE SET
                currency=EXCLUDED.currency,
                median_sale_price=EXCLUDED.median_sale_price,
                annual_growth_percent=EXCLUDED.annual_growth_percent,
                metadata=EXCLUDED.metadata
            "#,
        )
        .bind(provider_id)
        .execute(&mut *transaction)
        .await?
        .rows_affected();

        transaction.commit().await?;
        Ok(HmlrMarketAggregationReport {
            locations_created,
            markets_created,
            observations_upserted,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HmlrMarketAggregationReport {
    pub locations_created: u64,
    pub markets_created: u64,
    pub observations_upserted: u64,
}
