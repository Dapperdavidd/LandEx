use landex_api::{alerts::AlertEvaluationService, config::Config, state::AppState};
use std::error::Error;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let state = AppState::initialize(&Config::from_env()?).await?;
    let report = AlertEvaluationService::new(state.database)
        .evaluate()
        .await?;
    println!(
        "evaluated {} alert rules; initialized={}, emitted={}, unavailable={}",
        report.evaluated, report.initialized, report.emitted, report.unavailable
    );
    Ok(())
}
