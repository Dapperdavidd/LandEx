use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

#[derive(Clone)]
pub struct LocationRepository {
    pool: PgPool,
}

impl LocationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn search(
        &self,
        filters: &LocationSearchFilters,
    ) -> Result<Vec<LocationSummary>, sqlx::Error> {
        let mut query = QueryBuilder::<Postgres>::new(
            r#"
            SELECT
                loc.id,
                loc.parent_id,
                loc.kind,
                loc.name,
                loc.country_code,
                loc.administrative_code,
                loc.latitude,
                loc.longitude,
                loc.population,
                (SELECT COUNT(*) FROM properties p WHERE p.location_id = loc.id) AS property_count,
                (SELECT COUNT(*) FROM markets m WHERE m.location_id = loc.id) AS market_count
            FROM locations loc
            WHERE TRUE
            "#,
        );

        if let Some(query_text) = &filters.query {
            query.push(" AND loc.normalized_name ILIKE ");
            query.push_bind(format!("%{query_text}%"));
        }
        if let Some(country_code) = &filters.country_code {
            query.push(" AND loc.country_code = ");
            query.push_bind(country_code);
        }
        if let Some(kind) = &filters.kind {
            query.push(" AND loc.kind = ");
            query.push_bind(kind);
        }

        query.push(
            " ORDER BY property_count DESC, market_count DESC, loc.name ASC, loc.id ASC LIMIT ",
        );
        query.push_bind(filters.limit);

        query
            .build_query_as::<LocationSummary>()
            .fetch_all(&self.pool)
            .await
    }

    pub async fn find_with_hierarchy(
        &self,
        id: Uuid,
    ) -> Result<Option<LocationDetail>, sqlx::Error> {
        let mut hierarchy = sqlx::query_as::<_, LocationHierarchyRow>(
            r#"
            WITH RECURSIVE hierarchy AS (
                SELECT
                    id,
                    parent_id,
                    kind,
                    name,
                    country_code,
                    administrative_code,
                    latitude,
                    longitude,
                    population,
                    0 AS depth
                FROM locations
                WHERE id = $1

                UNION ALL

                SELECT
                    parent.id,
                    parent.parent_id,
                    parent.kind,
                    parent.name,
                    parent.country_code,
                    parent.administrative_code,
                    parent.latitude,
                    parent.longitude,
                    parent.population,
                    child.depth + 1
                FROM locations parent
                INNER JOIN hierarchy child ON child.parent_id = parent.id
            )
            SELECT * FROM hierarchy ORDER BY depth ASC
            "#,
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;

        let Some(target) = hierarchy.first().cloned() else {
            return Ok(None);
        };
        hierarchy.reverse();

        Ok(Some(LocationDetail {
            location: target.into(),
            hierarchy: hierarchy.into_iter().map(LocationNode::from).collect(),
        }))
    }
}

#[derive(Clone, Debug)]
pub struct LocationSearchFilters {
    pub query: Option<String>,
    pub country_code: Option<String>,
    pub kind: Option<String>,
    pub limit: i64,
}

#[derive(Debug, FromRow, Serialize)]
pub struct LocationSummary {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: String,
    pub name: String,
    pub country_code: String,
    pub administrative_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub population: Option<i64>,
    pub property_count: i64,
    pub market_count: i64,
}

#[derive(Clone, Debug, FromRow)]
struct LocationHierarchyRow {
    id: Uuid,
    parent_id: Option<Uuid>,
    kind: String,
    name: String,
    country_code: String,
    administrative_code: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    population: Option<i64>,
    #[allow(dead_code)]
    depth: i32,
}

#[derive(Debug, Serialize)]
pub struct LocationDetail {
    pub location: LocationNode,
    pub hierarchy: Vec<LocationNode>,
}

#[derive(Debug, Serialize)]
pub struct LocationNode {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: String,
    pub name: String,
    pub country_code: String,
    pub administrative_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub population: Option<i64>,
}

impl From<LocationHierarchyRow> for LocationNode {
    fn from(row: LocationHierarchyRow) -> Self {
        Self {
            id: row.id,
            parent_id: row.parent_id,
            kind: row.kind,
            name: row.name,
            country_code: row.country_code,
            administrative_code: row.administrative_code,
            latitude: row.latitude,
            longitude: row.longitude,
            population: row.population,
        }
    }
}
