use landex_api::{config::Config, scoring::ScoreRefreshService, state::AppState};
use std::error::Error;
#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let state = AppState::initialize(&Config::from_env()?).await?;
    let report = ScoreRefreshService::new(state.database).refresh().await?;
    println!(
        "recorded {} property scores and {} market scores; unavailable={}",
        report.properties_recorded, report.markets_recorded, report.unavailable
    );
    Ok(())
}
