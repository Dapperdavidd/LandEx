use landex_api::{config::Config, paper::PortfolioRefreshService, state::AppState};
use std::error::Error;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let state = AppState::initialize(&Config::from_env()?).await?;
    let report = PortfolioRefreshService::new(state.database)
        .refresh()
        .await?;
    println!(
        "recorded {} portfolio valuations; skipped={}",
        report.accounts_recorded, report.accounts_skipped
    );
    Ok(())
}
