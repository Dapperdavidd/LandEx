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

    pub const fn hierarchy_depth(self) -> u8 {
        match self {
            Self::Country => 0,
            Self::Region => 1,
            Self::City => 2,
            Self::District => 3,
            Self::Neighborhood => 4,
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
