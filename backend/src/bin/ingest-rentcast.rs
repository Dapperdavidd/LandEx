use std::{env, error::Error};

use landex_api::{
    config::Config,
    ingestion::{IngestionService, RentCastProvider, RentCastScope},
    state::AppState,
};
use tracing::info;
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("landex_api=info")),
        )
        .init();

    let config = Config::from_env()?;
    let state = AppState::initialize(&config).await?;
    let provider = RentCastProvider::new(
        required_env("RENTCAST_API_KEY")?,
        RentCastScope {
            city: optional_env("RENTCAST_CITY"),
            state: optional_env("RENTCAST_STATE"),
            zip_code: optional_env("RENTCAST_ZIP_CODE"),
        },
    )?;
    let max_pages =
        optional_env("RENTCAST_MAX_PAGES").map_or(Ok(1), |value| value.parse::<usize>())?;
    if max_pages == 0 {
        return Err("RENTCAST_MAX_PAGES must be greater than zero".into());
    }

    let report = IngestionService::new(state.database)
        .run(&provider, max_pages)
        .await?;

    info!(
        pages = report.pages,
        locations = report.locations,
        properties = report.properties,
        listings = report.listings,
        has_more = report.has_more,
        "RentCast ingestion completed"
    );

    Ok(())
}

fn required_env(key: &'static str) -> Result<String, Box<dyn Error>> {
    optional_env(key)
        .ok_or_else(|| format!("required environment variable {key} is missing").into())
}

fn optional_env(key: &'static str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}
