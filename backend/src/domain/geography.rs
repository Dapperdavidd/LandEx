use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocationKind {
    Country,
    Region,
    City,
    District,
    Neighborhood,
}

impl LocationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Country => "country",
            Self::Region => "region",
            Self::City => "city",
            Self::District => "district",
            Self::Neighborhood => "neighborhood",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Location {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub kind: LocationKind,
    pub name: String,
    pub normalized_name: String,
    pub country_code: String,
    pub administrative_code: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub population: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
