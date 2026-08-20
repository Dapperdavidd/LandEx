use std::{env, io};

use landex_api::{
    config::Config,
    fx::{frankfurter::FrankfurterClient, repository::CurrencyRateRepository},
    state::AppState,
};

#[actix_web::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();
    let config = Config::from_env().map_err(io::Error::other)?;
    let state = AppState::initialize(&config)
        .await
        .map_err(io::Error::other)?;
    let base = env::var("FX_BASE_CURRENCY")
        .unwrap_or_else(|_| "USD".to_owned())
        .to_uppercase();
    let quotes = env::var("FX_QUOTE_CURRENCIES")
        .unwrap_or_else(|_| "EUR,GBP,NGN,AED,CAD,AUD".to_owned())
        .split(',')
        .map(|value| value.trim().to_uppercase())
        .filter(|value| value.len() == 3 && value != &base)
        .collect::<Vec<_>>();
    if quotes.is_empty() {
        return Err(io::Error::other(
            "FX_QUOTE_CURRENCIES contains no valid quotes",
        ));
    }
    let rates = FrankfurterClient::new()
        .map_err(io::Error::other)?
        .latest(&base, &quotes)
        .await
        .map_err(io::Error::other)?;
    let affected = CurrencyRateRepository::new(state.database)
        .upsert(&rates)
        .await
        .map_err(io::Error::other)?;
    println!("stored {affected} dated currency rates");
    Ok(())
}
