use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Clone)]
pub struct LocationIntelligenceRepository {
    pool: PgPool,
}

impl LocationIntelligenceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn for_property(
        &self,
        property_id: Uuid,
        radius_meters: i32,
        limit: i64,
    ) -> Result<Option<PropertyLocationIntelligence>, sqlx::Error> {
        let property = sqlx::query_as::<_, PropertyCoordinates>(
            "SELECT latitude, longitude FROM properties WHERE id = $1",
        )
        .bind(property_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(property) = property else {
            return Ok(None);
        };

        let features = sqlx::query_as::<_, NearbyFeature>(
            r#"
            SELECT nf.id, nf.category, nf.kind, nf.name, nf.latitude, nf.longitude,
                   pnf.distance_meters, pnf.observed_at, pnf.expires_at
            FROM property_nearby_features pnf
            JOIN nearby_features nf ON nf.id = pnf.feature_id
            WHERE pnf.property_id = $1 AND pnf.distance_meters <= $2
            ORDER BY pnf.distance_meters, nf.id
            LIMIT $3
            "#,
        )
        .bind(property_id)
        .bind(radius_meters)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let categories = sqlx::query_as::<_, CategorySummary>(
            r#"
            SELECT nf.category, COUNT(*)::BIGINT AS feature_count,
                   MIN(pnf.distance_meters)::INTEGER AS nearest_distance_meters
            FROM property_nearby_features pnf
            JOIN nearby_features nf ON nf.id = pnf.feature_id
            WHERE pnf.property_id = $1 AND pnf.distance_meters <= $2
            GROUP BY nf.category
            ORDER BY nf.category
            "#,
        )
        .bind(property_id)
        .bind(radius_meters)
        .fetch_all(&self.pool)
        .await?;

        let cache = sqlx::query_as::<_, CacheStatus>(
            r#"
            SELECT MAX(observed_at) AS observed_at, MIN(expires_at) AS expires_at,
                   BOOL_AND(expires_at > NOW()) AS fresh
            FROM property_nearby_features
            WHERE property_id = $1 AND distance_meters <= $2
            "#,
        )
        .bind(property_id)
        .bind(radius_meters)
        .fetch_one(&self.pool)
        .await?;

        Ok(Some(PropertyLocationIntelligence {
            property_id,
            property_latitude: property.latitude,
            property_longitude: property.longitude,
            radius_meters,
            cache: LocationIntelligenceCache {
                populated: cache.observed_at.is_some(),
                fresh: cache.fresh.unwrap_or(false),
                observed_at: cache.observed_at,
                expires_at: cache.expires_at,
            },
            categories,
            features,
        }))
    }

    pub async fn property_coordinates(
        &self,
        property_id: Uuid,
    ) -> Result<Option<PropertyCoordinates>, sqlx::Error> {
        sqlx::query_as("SELECT latitude, longitude FROM properties WHERE id = $1")
            .bind(property_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn replace_property_features(
        &self,
        property_id: Uuid,
        radius_meters: i32,
        expires_at: DateTime<Utc>,
        features: &[NearbyFeatureInput],
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM property_nearby_features WHERE property_id = $1 AND query_radius_meters = $2",
        )
        .bind(property_id)
        .bind(radius_meters)
        .execute(&mut *transaction)
        .await?;

        for feature in features {
            let feature_id: Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO nearby_features (
                    source, source_element_type, source_id, category, kind, name, latitude, longitude, tags
                ) VALUES ('openstreetmap', $1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (source, source_element_type, source_id) DO UPDATE SET
                    category = EXCLUDED.category, kind = EXCLUDED.kind, name = EXCLUDED.name,
                    latitude = EXCLUDED.latitude, longitude = EXCLUDED.longitude, tags = EXCLUDED.tags,
                    updated_at = NOW()
                RETURNING id
                "#,
            )
            .bind(&feature.source_element_type)
            .bind(feature.source_id)
            .bind(&feature.category)
            .bind(&feature.kind)
            .bind(&feature.name)
            .bind(feature.latitude)
            .bind(feature.longitude)
            .bind(&feature.tags)
            .fetch_one(&mut *transaction)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO property_nearby_features (
                    property_id, feature_id, distance_meters, query_radius_meters, expires_at
                ) VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(property_id)
            .bind(feature_id)
            .bind(feature.distance_meters)
            .bind(radius_meters)
            .bind(expires_at)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await
    }
}

#[derive(Debug, FromRow)]
pub struct PropertyCoordinates {
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

impl PropertyCoordinates {
    pub fn complete(&self) -> Option<(f64, f64)> {
        self.latitude.zip(self.longitude)
    }
}

#[derive(Clone, Debug)]
pub struct NearbyFeatureInput {
    pub source_element_type: String,
    pub source_id: i64,
    pub category: String,
    pub kind: String,
    pub name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub tags: Value,
    pub distance_meters: i32,
}

#[derive(Debug, FromRow, Serialize)]
pub struct NearbyFeature {
    pub id: Uuid,
    pub category: String,
    pub kind: String,
    pub name: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub distance_meters: i32,
    pub observed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Serialize)]
pub struct CategorySummary {
    pub category: String,
    pub feature_count: i64,
    pub nearest_distance_meters: i32,
}

#[derive(Debug, FromRow)]
struct CacheStatus {
    observed_at: Option<DateTime<Utc>>,
    expires_at: Option<DateTime<Utc>>,
    fresh: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct LocationIntelligenceCache {
    pub populated: bool,
    pub fresh: bool,
    pub observed_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct PropertyLocationIntelligence {
    pub property_id: Uuid,
    pub property_latitude: Option<f64>,
    pub property_longitude: Option<f64>,
    pub radius_meters: i32,
    pub cache: LocationIntelligenceCache,
    pub categories: Vec<CategorySummary>,
    pub features: Vec<NearbyFeature>,
}
