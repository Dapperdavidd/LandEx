use std::error::Error;

use landex_api::{
    config::Config,
    ingestion::{CountriesDevCityProvider, IngestionService},
    state::AppState,
};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;
    let state = AppState::initialize(&config).await?;
    let countries: Vec<String> = sqlx::query_scalar("SELECT cc.country_code FROM country_coverage cc WHERE cc.has_market_data AND NOT EXISTS (SELECT 1 FROM provider_locations pl JOIN providers p ON p.id=pl.provider_id JOIN locations l ON l.id=pl.location_id WHERE p.slug='countries-dev' AND l.kind='city' AND l.country_code=cc.country_code) ORDER BY cc.country_code")
    .fetch_all(&state.database)
    .await?;
    let mut locations = 0usize;
    let mut completed = 0usize;
    let mut skipped = Vec::new();
    for country in countries {
        match IngestionService::new(state.database.clone())
            .run(&CountriesDevCityProvider::new(&country)?, 1)
            .await
        {
            Ok(report) => {
                locations += report.locations;
                completed += 1;
            }
            Err(error) => skipped.push(format!("{country}: {error}")),
        }
    }
    println!(
        "ingested {locations} city references across {completed} market countries; skipped={}",
        skipped.len()
    );
    for message in skipped {
        eprintln!("{message}");
    }
    Ok(())
}
