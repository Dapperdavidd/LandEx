use std::{env, fs, io, path::Path, time::Duration};

use landex_api::{config::Config, state::AppState};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

const DEFAULT_URL: &str = "https://www.sec.gov/files/company_tickers_exchange.json";
const SOURCE_PAGE: &str =
    "https://www.sec.gov/search-filings/edgar-search-assistance/accessing-edgar-data";
const DEFAULT_CACHE_PATH: &str = ".data/sec/company-tickers-exchange.json";

#[derive(Debug, Clone, PartialEq)]
struct ListedReit {
    cik: i64,
    name: String,
    ticker: String,
    exchange: String,
}

#[derive(Debug, Deserialize)]
struct SecDataset {
    fields: Vec<String>,
    data: Vec<Vec<Value>>,
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();
    let config = Config::from_env().map_err(io::Error::other)?;
    let state = AppState::initialize(&config)
        .await
        .map_err(io::Error::other)?;
    let user_agent = env::var("SEC_EDGAR_USER_AGENT").map_err(|_| {
        io::Error::other(
            "SEC_EDGAR_USER_AGENT is required and must identify the application plus a contact email",
        )
    })?;
    if !user_agent.contains('@') {
        return Err(io::Error::other(
            "SEC_EDGAR_USER_AGENT must include a contact email for SEC fair-access compliance",
        ));
    }
    let source_url = env::var("SEC_TICKER_EXCHANGE_URL").unwrap_or_else(|_| DEFAULT_URL.into());
    let cache_path =
        env::var("SEC_TICKER_EXCHANGE_CACHE_PATH").unwrap_or_else(|_| DEFAULT_CACHE_PATH.into());
    let bytes = load_dataset(&source_url, &cache_path, &user_agent).await?;
    let dataset: SecDataset = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
    let reits = extract_explicit_reits(dataset)?;
    if reits.len() < 20 {
        return Err(io::Error::other(format!(
            "SEC snapshot produced only {} explicit REIT names; refusing a likely incomplete import",
            reits.len()
        )));
    }

    let mut tx = state.database.begin().await.map_err(io::Error::other)?;
    let provider_id = upsert_provider(&mut tx).await.map_err(io::Error::other)?;
    upsert_instruments(&mut tx, provider_id, &reits)
        .await
        .map_err(io::Error::other)?;
    tx.commit().await.map_err(io::Error::other)?;
    println!(
        "stored {} SEC-verified listed REIT research instruments (no quote data)",
        reits.len()
    );
    Ok(())
}

async fn load_dataset(source_url: &str, cache_path: &str, user_agent: &str) -> io::Result<Vec<u8>> {
    let path = Path::new(cache_path);
    let refresh = env::var("SEC_TICKER_EXCHANGE_REFRESH")
        .ok()
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));
    let fresh_cache = path.metadata().ok().and_then(|metadata| {
        metadata
            .modified()
            .ok()?
            .elapsed()
            .ok()
            .filter(|age| *age < Duration::from_secs(7 * 24 * 60 * 60))?;
        (metadata.len() > 100_000).then_some(())
    });
    if fresh_cache.is_some() && !refresh {
        return fs::read(path);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let response = reqwest::Client::new()
        .get(source_url)
        .header(reqwest::header::USER_AGENT, user_agent)
        .header(reqwest::header::ACCEPT_ENCODING, "gzip, deflate")
        .send()
        .await
        .map_err(io::Error::other)?
        .error_for_status()
        .map_err(io::Error::other)?;
    let bytes = response.bytes().await.map_err(io::Error::other)?.to_vec();
    if bytes.len() < 100_000 {
        return Err(io::Error::other(
            "SEC ticker response is unexpectedly small",
        ));
    }
    fs::write(path, &bytes)?;
    Ok(bytes)
}

fn extract_explicit_reits(dataset: SecDataset) -> io::Result<Vec<ListedReit>> {
    let index = |field: &str| {
        dataset
            .fields
            .iter()
            .position(|candidate| candidate == field)
            .ok_or_else(|| io::Error::other(format!("SEC dataset is missing {field}")))
    };
    let cik_index = index("cik")?;
    let name_index = index("name")?;
    let ticker_index = index("ticker")?;
    let exchange_index = index("exchange")?;
    let mut reits = dataset
        .data
        .into_iter()
        .filter_map(|row| {
            let name = row.get(name_index)?.as_str()?.trim();
            let uppercase = name.to_ascii_uppercase();
            let explicitly_reit = uppercase.contains(" REIT")
                || uppercase.starts_with("REIT ")
                || uppercase.contains("REAL ESTATE INVESTMENT TRUST");
            if !explicitly_reit {
                return None;
            }
            let ticker = row.get(ticker_index)?.as_str()?.trim();
            let exchange = row.get(exchange_index)?.as_str()?.trim();
            if ticker.is_empty() || exchange.is_empty() {
                return None;
            }
            Some(ListedReit {
                cik: row.get(cik_index)?.as_i64()?,
                name: name.to_owned(),
                ticker: ticker.to_owned(),
                exchange: exchange.to_owned(),
            })
        })
        .collect::<Vec<_>>();
    reits.sort_by(|a, b| a.name.cmp(&b.name).then(a.ticker.cmp(&b.ticker)));
    reits.dedup_by(|a, b| a.cik == b.cik && a.ticker == b.ticker);
    Ok(reits)
}

async fn upsert_provider(tx: &mut Transaction<'_, Postgres>) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO providers (slug,name) VALUES ('sec-edgar-listed-reits','SEC EDGAR Listed REIT Research') ON CONFLICT (slug) DO UPDATE SET updated_at=NOW() RETURNING id",
    )
    .fetch_one(&mut **tx)
    .await
}

async fn upsert_instruments(
    tx: &mut Transaction<'_, Postgres>,
    provider_id: Uuid,
    reits: &[ListedReit],
) -> Result<(), sqlx::Error> {
    for chunk in reits.chunks(250) {
        let mut query = QueryBuilder::<Postgres>::new(
            "INSERT INTO investment_instruments (slug,name,instrument_kind,status,country_code,currency,symbol,exchange,provider_id,source_url,valuation_method,liquidity_class,metadata) ",
        );
        query.push_values(chunk, |mut values, reit| {
            let slug = format!("sec-{}-{}", reit.cik, reit.ticker.to_ascii_lowercase());
            let filing_url = format!(
                "https://www.sec.gov/edgar/browse/?CIK={}&owner=exclude",
                reit.cik
            );
            values
                .push_bind(slug)
                .push_bind(&reit.name)
                .push_bind("listed_security")
                .push_bind("research")
                .push_bind("US")
                .push_bind("USD")
                .push_bind(&reit.ticker)
                .push_bind(&reit.exchange)
                .push_bind(provider_id)
                .push_bind(filing_url)
                .push_bind("SEC-verified issuer, ticker, and exchange identity; no quote or return series is asserted")
                .push_bind("listed")
                .push_bind(json!({
                    "cik": reit.cik,
                    "classification_basis": "Issuer name explicitly identifies a REIT in the SEC ticker/exchange dataset",
                    "identity_source": SOURCE_PAGE,
                    "quote_coverage": "unavailable",
                    "paper_tradeable": false
                }));
        });
        query.push(
            " ON CONFLICT (slug) DO UPDATE SET name=EXCLUDED.name,symbol=EXCLUDED.symbol,exchange=EXCLUDED.exchange,provider_id=EXCLUDED.provider_id,source_url=EXCLUDED.source_url,valuation_method=EXCLUDED.valuation_method,metadata=EXCLUDED.metadata,updated_at=NOW()",
        );
        query.build().execute(&mut **tx).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_explicit_reit_names_with_complete_market_identity() {
        let dataset = SecDataset {
            fields: vec!["exchange", "ticker", "name", "cik"]
                .into_iter()
                .map(String::from)
                .collect(),
            data: vec![
                vec![
                    json!("NYSE"),
                    json!("REAL"),
                    json!("EXAMPLE REIT INC"),
                    json!(42),
                ],
                vec![
                    json!("NYSE"),
                    json!("HOME"),
                    json!("EXAMPLE HOMES INC"),
                    json!(43),
                ],
                vec![
                    json!(""),
                    json!("MISS"),
                    json!("MISSING EXCHANGE REIT"),
                    json!(44),
                ],
            ],
        };
        let rows = extract_explicit_reits(dataset).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ticker, "REAL");
        assert_eq!(rows[0].exchange, "NYSE");
    }
}
