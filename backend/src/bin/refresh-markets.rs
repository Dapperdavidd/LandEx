use std::error::Error;

use chrono::Utc;
use landex_api::{config::Config, market::MarketAggregationService, state::AppState};
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
    let report = MarketAggregationService::new(state.database)
        .refresh(Utc::now().date_naive())
        .await?;

    info!(
        markets_affected = report.markets_affected,
        observations_upserted = report.observations_upserted,
        observed_on = %report.observed_on,
        "market aggregation completed"
    );

    Ok(())
}
