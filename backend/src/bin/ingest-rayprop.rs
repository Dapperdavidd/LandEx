use landex_api::{
    config::Config,
    ingestion::{IngestionService, RayPropProvider},
    state::AppState,
};
use std::{env, error::Error};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;
    let state = AppState::initialize(&config).await?;
    let key = env::var("RAYPROP_API_KEY")?;
    let city = env::var("RAYPROP_CITY").unwrap_or_else(|_| "Lagos".to_owned());
    let max_pages = env::var("RAYPROP_MAX_PAGES")
        .ok()
        .map_or(Ok(1), |value| value.parse::<usize>())?;
    if max_pages == 0 {
        return Err("RAYPROP_MAX_PAGES must be greater than zero".into());
    }
    let report = IngestionService::new(state.database)
        .run(&RayPropProvider::new(key, city)?, max_pages)
        .await?;
    println!(
        "ingested {} RayProp pages, {} properties, {} shortlet listings; has_more={}",
        report.pages, report.properties, report.listings, report.has_more
    );
    Ok(())
}
