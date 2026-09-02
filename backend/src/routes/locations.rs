use actix_web::{HttpResponse, get, web};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    error::ApiError,
    repository::location::{LocationRepository, LocationSearchFilters},
    state::AppState,
};

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 300;

#[derive(Debug, Deserialize)]
pub struct LocationSearchQuery {
    q: Option<String>,
    country_code: Option<String>,
    kind: Option<String>,
    limit: Option<i64>,
}

#[get("/locations")]
pub async fn list_locations(
    state: web::Data<AppState>,
    query: web::Query<LocationSearchQuery>,
) -> Result<HttpResponse, ApiError> {
    let filters = query.into_inner().into_filters()?;
    let locations = LocationRepository::new(state.database.clone())
        .search(&filters)
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?;

    Ok(HttpResponse::Ok().json(locations))
}

#[get("/locations/{id}")]
pub async fn get_location(
    state: web::Data<AppState>,
    id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let location = LocationRepository::new(state.database.clone())
        .find_with_hierarchy(id.into_inner())
        .await
        .map_err(|_| ApiError::ServiceUnavailable)?
        .ok_or(ApiError::NotFound)?;

    Ok(HttpResponse::Ok().json(location))
}

impl LocationSearchQuery {
    fn into_filters(self) -> Result<LocationSearchFilters, ApiError> {
        let limit = self.limit.unwrap_or(DEFAULT_LIMIT);
        if !(1..=MAX_LIMIT).contains(&limit) {
            return Err(ApiError::InvalidRequest(format!(
                "limit must be between 1 and {MAX_LIMIT}"
            )));
        }

        let query = self
            .q
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty());
        if query.as_ref().is_some_and(|value| value.len() < 2) {
            return Err(ApiError::InvalidRequest(
                "q must contain at least two characters".to_owned(),
            ));
        }

        Ok(LocationSearchFilters {
            query,
            country_code: normalize_country_code(self.country_code)?,
            kind: normalize_kind(self.kind)?,
            limit,
        })
    }
}

fn normalize_country_code(value: Option<String>) -> Result<Option<String>, ApiError> {
    value
        .map(|value| {
            let normalized = value.trim().to_ascii_uppercase();
            if normalized.len() != 2 || !normalized.bytes().all(|byte| byte.is_ascii_alphabetic()) {
                return Err(ApiError::InvalidRequest(
                    "country_code must contain exactly two letters".to_owned(),
                ));
            }
            Ok(normalized)
        })
        .transpose()
}

fn normalize_kind(value: Option<String>) -> Result<Option<String>, ApiError> {
    const KINDS: &[&str] = &["country", "region", "city", "district", "neighborhood"];
    value
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            if !KINDS.contains(&normalized.as_str()) {
                return Err(ApiError::InvalidRequest(format!(
                    "unsupported location kind: {value}"
                )));
            }
            Ok(normalized)
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::LocationSearchQuery;

    #[test]
    fn normalizes_location_filters() {
        let filters = LocationSearchQuery {
            q: Some(" Lagos ".to_owned()),
            country_code: Some("ng".to_owned()),
            kind: Some("City".to_owned()),
            limit: None,
        }
        .into_filters()
        .expect("valid filters");

        assert_eq!(filters.query.as_deref(), Some("lagos"));
        assert_eq!(filters.country_code.as_deref(), Some("NG"));
        assert_eq!(filters.kind.as_deref(), Some("city"));
        assert_eq!(filters.limit, 20);
    }
}
