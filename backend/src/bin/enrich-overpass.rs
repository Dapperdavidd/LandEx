use std::{env, error::Error};

use landex_api::{
    config::Config,
    location_intelligence::{OverpassEnrichmentService, OverpassProvider},
    state::AppState,
};
use uuid::Uuid;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;
    let state = AppState::initialize(&config).await?;
    let property_id = env::var("OVERPASS_PROPERTY_ID")?.parse::<Uuid>()?;
    let radius_meters = env::var("OVERPASS_RADIUS_METERS")
        .ok()
        .map_or(Ok(1_000), |value| value.parse::<i32>())?;

    let report = OverpassEnrichmentService::new(state.database, OverpassProvider::new()?)
        .enrich_property(property_id, radius_meters)
        .await?;
    println!(
        "enriched property {} with {} nearby OpenStreetMap features within {}m; cache expires {}",
        report.property_id, report.feature_count, report.radius_meters, report.expires_at
    );
    Ok(())
}
