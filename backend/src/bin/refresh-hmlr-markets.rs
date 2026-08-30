use std::error::Error;

use landex_api::{
    config::Config, historical::aggregation::HmlrMarketAggregationService, state::AppState,
};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let state = AppState::initialize(&Config::from_env()?).await?;
    let report = HmlrMarketAggregationService::new(state.database)
        .refresh()
        .await?;
    println!(
        "HMLR aggregation complete: locations={}, markets={}, monthly observations={}",
        report.locations_created, report.markets_created, report.observations_upserted
    );
    Ok(())
}
