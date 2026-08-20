use reqwest::Client;
use rust_decimal::Decimal;
use serde::Deserialize;
use thiserror::Error;

use super::CurrencyRate;

const DEFAULT_ENDPOINT: &str = "https://api.frankfurter.dev/v2/rates";

pub struct FrankfurterClient {
    client: Client,
    endpoint: String,
}

impl FrankfurterClient {
    pub fn new() -> Result<Self, FrankfurterError> {
        Ok(Self {
            client: Client::builder()
                .user_agent("LandEX/0.1 currency-rate ingestion")
                .build()?,
            endpoint: DEFAULT_ENDPOINT.to_owned(),
        })
    }

    pub async fn latest(
        &self,
        base: &str,
        quotes: &[String],
    ) -> Result<Vec<CurrencyRate>, FrankfurterError> {
        let response = self
            .client
            .get(&self.endpoint)
            .query(&[("base", base), ("quotes", &quotes.join(","))])
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<FrankfurterRate>>()
            .await?;
        Ok(map_rates(response))
    }
}

fn map_rates(response: Vec<FrankfurterRate>) -> Vec<CurrencyRate> {
    response
        .into_iter()
        .map(|rate| CurrencyRate {
            provider: format!(
                "frankfurter:{}",
                rate.provider.unwrap_or_else(|| "default".to_owned())
            ),
            base_currency: rate.base,
            quote_currency: rate.quote,
            rate: rate.rate,
            observed_on: rate.date,
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct FrankfurterRate {
    date: chrono::NaiveDate,
    base: String,
    quote: String,
    rate: Decimal,
    provider: Option<String>,
}

#[derive(Debug, Error)]
pub enum FrankfurterError {
    #[error("could not call the currency-rate provider: {0}")]
    Request(#[from] reqwest::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_dated_provider_rates() {
        let response: Vec<FrankfurterRate> = serde_json::from_value(serde_json::json!([{
            "date": "2026-08-20", "base": "USD", "quote": "NGN",
            "rate": "1532.5", "provider": "CBN"
        }]))
        .expect("fixture response");
        let rates = map_rates(response);
        assert_eq!(rates.len(), 1);
        assert_eq!(rates[0].provider, "frankfurter:CBN");
        assert_eq!(rates[0].rate, Decimal::new(15325, 1));
    }
}
