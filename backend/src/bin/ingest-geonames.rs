use std::{env, error::Error};

use landex_api::{
    config::Config,
    ingestion::{GeoNamesProvider, IngestionService},
    state::AppState,
};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;
    let state = AppState::initialize(&config).await?;
    let username = env::var("GEONAMES_USERNAME")?;
    let query = env::var("GEONAMES_QUERY").unwrap_or_else(|_| "Lagos".to_owned());
    let country_code = env::var("GEONAMES_COUNTRY_CODE")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let max_rows = env::var("GEONAMES_MAX_ROWS")
        .ok()
        .map_or(Ok(100), |value| value.parse::<u16>())?;
    let max_pages = env::var("GEONAMES_MAX_PAGES")
        .ok()
        .map_or(Ok(1), |value| value.parse::<usize>())?;
    if max_pages == 0 {
        return Err("GEONAMES_MAX_PAGES must be greater than zero".into());
    }

    let report = IngestionService::new(state.database)
        .run(
            &GeoNamesProvider::new(username, query, country_code, max_rows)?,
            max_pages,
        )
        .await?;
    println!(
        "ingested {} GeoNames pages and {} normalized locations; has_more={}",
        report.pages, report.locations, report.has_more
    );
    Ok(())
}
