use std::collections::{HashMap, HashSet};

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{
    IngestionError, PropertyProvider, ProviderListing, ProviderLocation, ProviderPage,
    ProviderProperty, RequestBudget,
};
use crate::domain::ListingType;

#[derive(Clone)]
pub struct IngestionService {
    pool: PgPool,
}

impl IngestionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn run(
        &self,
        provider: &dyn PropertyProvider,
        max_pages: usize,
    ) -> Result<IngestionReport, IngestionError> {
        let provider_id = self.upsert_provider(provider.slug()).await?;
        let mut cursor = None;
        let mut seen_cursors = HashSet::new();
        let mut report = IngestionReport::default();

        for _ in 0..max_pages {
            let attempt_id = match provider.request_budget() {
                Some(budget) => Some(
                    self.reserve_request(provider_id, provider.slug(), cursor.as_deref(), budget)
                        .await?,
                ),
                None => None,
            };
            let page = match provider.fetch_page(cursor.as_deref()).await {
                Ok(page) => {
                    if let Some(attempt_id) = attempt_id {
                        self.record_request_outcome(attempt_id, "succeeded").await?;
                    }
                    page
                }
                Err(error) => {
                    if let Some(attempt_id) = attempt_id {
                        let _ = self.record_request_outcome(attempt_id, "failed").await;
                    }
                    return Err(error);
                }
            };
            page.validate()?;

            let page_report = self.persist_page(provider_id, &page).await?;
            report.pages += 1;
            report.locations += page_report.locations;
            report.properties += page_report.properties;
            report.listings += page_report.listings;

            let Some(next_cursor) = page.next_cursor else {
                return Ok(report);
            };
            if !seen_cursors.insert(next_cursor.clone()) {
                return Err(IngestionError::RepeatedCursor);
            }
            cursor = Some(next_cursor);
        }

        report.has_more = cursor.is_some();
        Ok(report)
    }

    async fn upsert_provider(&self, slug: &str) -> Result<Uuid, sqlx::Error> {
        sqlx::query_scalar(
            r#"
            INSERT INTO providers (slug, name)
            VALUES ($1, $1)
            ON CONFLICT (slug) DO UPDATE
            SET updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(slug)
        .fetch_one(&self.pool)
        .await
    }

    async fn reserve_request(
        &self,
        provider_id: Uuid,
        provider_slug: &str,
        cursor: Option<&str>,
        budget: RequestBudget,
    ) -> Result<Uuid, IngestionError> {
        let mut transaction = self.pool.begin().await?;
        let lock_key = format!("provider-request-budget:{provider_slug}");
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(lock_key)
            .execute(&mut *transaction)
            .await?;

        let attempts: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM provider_request_attempts
            WHERE provider_id = $1
              AND requested_at >= NOW() - ($2 * INTERVAL '1 day')
            "#,
        )
        .bind(provider_id)
        .bind(i32::from(budget.window_days))
        .fetch_one(&mut *transaction)
        .await?;

        if attempts >= i64::from(budget.max_attempts) {
            return Err(IngestionError::RequestLimitReached {
                provider: provider_slug.to_owned(),
                attempts,
                limit: budget.max_attempts,
                window_days: budget.window_days,
            });
        }

        let attempt_id = sqlx::query_scalar(
            r#"
            INSERT INTO provider_request_attempts (provider_id, cursor)
            VALUES ($1, $2)
            RETURNING id
            "#,
        )
        .bind(provider_id)
        .bind(cursor)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;

        Ok(attempt_id)
    }

    async fn record_request_outcome(
        &self,
        attempt_id: Uuid,
        outcome: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE provider_request_attempts SET outcome = $2 WHERE id = $1")
            .bind(attempt_id)
            .bind(outcome)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn persist_page(
        &self,
        provider_id: Uuid,
        page: &ProviderPage,
    ) -> Result<IngestionReport, IngestionError> {
        let mut transaction = self.pool.begin().await?;
        let mut location_ids = HashMap::new();
        let mut property_ids = HashMap::new();

        let mut locations = page.locations.iter().collect::<Vec<_>>();
        locations.sort_by_key(|location| location.kind.hierarchy_depth());

        for location in locations {
            let location_id =
                persist_location(&mut transaction, provider_id, location, &location_ids).await?;
            location_ids.insert(location.source_id.clone(), location_id);
        }

        for property in &page.properties {
            let location_id = resolve_mapping(
                &mut transaction,
                provider_id,
                "location",
                &property.location_source_id,
                "provider_locations",
                "location_id",
                &location_ids,
            )
            .await?;
            let property_id =
                persist_property(&mut transaction, provider_id, property, location_id).await?;
            property_ids.insert(property.source_id.clone(), property_id);
        }

        for listing in &page.listings {
            let property_id = resolve_mapping(
                &mut transaction,
                provider_id,
                "property",
                &listing.property_source_id,
                "provider_properties",
                "property_id",
                &property_ids,
            )
            .await?;
            persist_listing(&mut transaction, provider_id, listing, property_id).await?;
        }

        transaction.commit().await?;

        Ok(IngestionReport {
            pages: 1,
            locations: page.locations.len(),
            properties: page.properties.len(),
            listings: page.listings.len(),
            has_more: page.next_cursor.is_some(),
        })
    }
}

async fn persist_location(
    transaction: &mut Transaction<'_, Postgres>,
    provider_id: Uuid,
    location: &ProviderLocation,
    page_location_ids: &HashMap<String, Uuid>,
) -> Result<Uuid, IngestionError> {
    let parent_id = match &location.parent_source_id {
        Some(source_id) => Some(
            resolve_mapping(
                transaction,
                provider_id,
                "parent location",
                source_id,
                "provider_locations",
                "location_id",
                page_location_ids,
            )
            .await?,
        ),
        None => None,
    };

    let location_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO locations (
            parent_id, kind, name, normalized_name, country_code,
            administrative_code, latitude, longitude, population
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        ON CONFLICT (parent_id, kind, normalized_name, country_code)
        DO UPDATE SET
            name = EXCLUDED.name,
            administrative_code = COALESCE(EXCLUDED.administrative_code, locations.administrative_code),
            latitude = COALESCE(EXCLUDED.latitude, locations.latitude),
            longitude = COALESCE(EXCLUDED.longitude, locations.longitude),
            population = COALESCE(EXCLUDED.population, locations.population),
            updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(parent_id)
    .bind(location.kind.as_str())
    .bind(&location.name)
    .bind(&location.normalized_name)
    .bind(&location.country_code)
    .bind(&location.administrative_code)
    .bind(location.latitude)
    .bind(location.longitude)
    .bind(location.population)
    .fetch_one(&mut **transaction)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO provider_locations (
            provider_id, location_id, source_id, raw_payload, last_seen_at
        )
        VALUES ($1, $2, $3, $4, NOW())
        ON CONFLICT (provider_id, source_id) DO UPDATE SET
            location_id = EXCLUDED.location_id,
            raw_payload = EXCLUDED.raw_payload,
            last_seen_at = NOW()
        "#,
    )
    .bind(provider_id)
    .bind(location_id)
    .bind(&location.source_id)
    .bind(&location.raw_payload)
    .execute(&mut **transaction)
    .await?;

    Ok(location_id)
}

async fn persist_property(
    transaction: &mut Transaction<'_, Postgres>,
    provider_id: Uuid,
    property: &ProviderProperty,
    location_id: Uuid,
) -> Result<Uuid, IngestionError> {
    let existing_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT property_id FROM provider_properties WHERE provider_id = $1 AND source_id = $2",
    )
    .bind(provider_id)
    .bind(&property.source_id)
    .fetch_optional(&mut **transaction)
    .await?;

    let property_id = if let Some(property_id) = existing_id {
        sqlx::query(
            r#"
            UPDATE properties SET
                location_id = $2,
                property_type = $3,
                address_line = $4,
                postal_code = $5,
                latitude = $6,
                longitude = $7,
                bedrooms = $8,
                bathrooms = $9,
                area_sqm = $10,
                lot_size_sqm = $11,
                year_built = $12,
                attributes = $13,
                updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(property_id)
        .bind(location_id)
        .bind(property.property_type.as_str())
        .bind(&property.address_line)
        .bind(&property.postal_code)
        .bind(property.latitude)
        .bind(property.longitude)
        .bind(property.bedrooms)
        .bind(property.bathrooms)
        .bind(property.area_sqm)
        .bind(property.lot_size_sqm)
        .bind(property.year_built)
        .bind(&property.attributes)
        .execute(&mut **transaction)
        .await?;
        property_id
    } else {
        sqlx::query_scalar(
            r#"
            INSERT INTO properties (
                location_id, property_type, address_line, postal_code, latitude,
                longitude, bedrooms, bathrooms, area_sqm, lot_size_sqm,
                year_built, attributes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id
            "#,
        )
        .bind(location_id)
        .bind(property.property_type.as_str())
        .bind(&property.address_line)
        .bind(&property.postal_code)
        .bind(property.latitude)
        .bind(property.longitude)
        .bind(property.bedrooms)
        .bind(property.bathrooms)
        .bind(property.area_sqm)
        .bind(property.lot_size_sqm)
        .bind(property.year_built)
        .bind(&property.attributes)
        .fetch_one(&mut **transaction)
        .await?
    };

    sqlx::query(
        r#"
        INSERT INTO provider_properties (
            provider_id, property_id, source_id, raw_payload, first_seen_at, last_seen_at
        )
        VALUES ($1, $2, $3, $4, NOW(), NOW())
        ON CONFLICT (provider_id, source_id) DO UPDATE SET
            property_id = EXCLUDED.property_id,
            raw_payload = EXCLUDED.raw_payload,
            last_seen_at = NOW()
        "#,
    )
    .bind(provider_id)
    .bind(property_id)
    .bind(&property.source_id)
    .bind(&property.raw_payload)
    .execute(&mut **transaction)
    .await?;

    Ok(property_id)
}

async fn persist_listing(
    transaction: &mut Transaction<'_, Postgres>,
    provider_id: Uuid,
    listing: &ProviderListing,
    property_id: Uuid,
) -> Result<(), IngestionError> {
    sqlx::query(
        r#"
        INSERT INTO listings (
            property_id, provider_id, source_id, listing_type, status, price,
            currency, listed_at, removed_at, source_url, raw_payload,
            first_seen_at, last_seen_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $12)
        ON CONFLICT (provider_id, source_id) DO UPDATE SET
            property_id = EXCLUDED.property_id,
            listing_type = EXCLUDED.listing_type,
            status = EXCLUDED.status,
            price = EXCLUDED.price,
            currency = EXCLUDED.currency,
            listed_at = EXCLUDED.listed_at,
            removed_at = EXCLUDED.removed_at,
            source_url = EXCLUDED.source_url,
            raw_payload = EXCLUDED.raw_payload,
            last_seen_at = EXCLUDED.last_seen_at
        "#,
    )
    .bind(property_id)
    .bind(provider_id)
    .bind(&listing.source_id)
    .bind(listing.listing_type.as_str())
    .bind(listing.status.as_str())
    .bind(listing.price)
    .bind(&listing.currency)
    .bind(listing.listed_at)
    .bind(listing.removed_at)
    .bind(&listing.source_url)
    .bind(&listing.raw_payload)
    .bind(listing.observed_at)
    .execute(&mut **transaction)
    .await?;

    let (asking_price, rental_price) = match listing.listing_type {
        ListingType::Sale => (Some(listing.price), None),
        ListingType::Rent => (None, Some(listing.price)),
    };

    sqlx::query(
        r#"
        INSERT INTO property_observations (
            property_id, provider_id, observed_on, asking_price,
            rental_price_monthly, currency, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (property_id, provider_id, observed_on) DO UPDATE SET
            asking_price = COALESCE(EXCLUDED.asking_price, property_observations.asking_price),
            rental_price_monthly = COALESCE(
                EXCLUDED.rental_price_monthly,
                property_observations.rental_price_monthly
            ),
            currency = EXCLUDED.currency,
            metadata = EXCLUDED.metadata
        "#,
    )
    .bind(property_id)
    .bind(provider_id)
    .bind(listing.observed_at.date_naive())
    .bind(asking_price)
    .bind(rental_price)
    .bind(&listing.currency)
    .bind(serde_json::json!({ "listing_source_id": listing.source_id }))
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn resolve_mapping(
    transaction: &mut Transaction<'_, Postgres>,
    provider_id: Uuid,
    entity: &'static str,
    source_id: &str,
    table: &str,
    id_column: &str,
    page_ids: &HashMap<String, Uuid>,
) -> Result<Uuid, IngestionError> {
    if let Some(id) = page_ids.get(source_id) {
        return Ok(*id);
    }

    let query =
        format!("SELECT {id_column} FROM {table} WHERE provider_id = $1 AND source_id = $2");
    sqlx::query_scalar(&query)
        .bind(provider_id)
        .bind(source_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| IngestionError::MissingReference {
            entity,
            source_id: source_id.to_owned(),
        })
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IngestionReport {
    pub pages: usize,
    pub locations: usize,
    pub properties: usize,
    pub listings: usize,
    pub has_more: bool,
}
