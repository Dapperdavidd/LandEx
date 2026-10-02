use std::{collections::HashMap, env, fs, io, path::Path, time::Duration};

use landex_api::{config::Config, state::AppState};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

const DEFAULT_URL: &str = "https://www.sec.gov/files/company_tickers_exchange.json";
const SOURCE_PAGE: &str =
    "https://www.sec.gov/search-filings/edgar-search-assistance/accessing-edgar-data";
const DEFAULT_CACHE_PATH: &str = ".data/sec/company-tickers-exchange.json";
const DEFAULT_SIC_URL: &str =
    "https://www.sec.gov/cgi-bin/browse-edgar?action=getcompany&SIC=6798&owner=include&count=100";
const DEFAULT_SIC_CACHE_DIR: &str = ".data/sec/sic-6798";

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
    let sic_url = env::var("SEC_REIT_SIC_URL").unwrap_or_else(|_| DEFAULT_SIC_URL.into());
    let sic_cache_dir =
        env::var("SEC_REIT_SIC_CACHE_DIR").unwrap_or_else(|_| DEFAULT_SIC_CACHE_DIR.into());
    let sic_entities = load_sic_entities(&sic_url, &sic_cache_dir, &user_agent).await?;
    let reits = extract_sic_reits(dataset, &sic_entities)?;
    if reits.len() < 100 {
        return Err(io::Error::other(format!(
            "SEC snapshots produced only {} exchange-listed SIC 6798 REIT securities; refusing a likely incomplete import",
            reits.len()
        )));
    }

    let mut tx = state.database.begin().await.map_err(io::Error::other)?;
    let provider_id = upsert_provider(&mut tx).await.map_err(io::Error::other)?;
    upsert_instruments(&mut tx, provider_id, &reits)
        .await
        .map_err(io::Error::other)?;
    restore_quote_metadata(&mut tx, provider_id)
        .await
        .map_err(io::Error::other)?;
    tx.commit().await.map_err(io::Error::other)?;
    println!(
        "stored {} SEC-verified listed REIT research instruments (no quote data)",
        reits.len()
    );
    Ok(())
}

async fn load_sic_entities(
    source_url: &str,
    cache_dir: &str,
    user_agent: &str,
) -> io::Result<HashMap<i64, String>> {
    let cache_dir = Path::new(cache_dir);
    fs::create_dir_all(cache_dir)?;
    let refresh = env::var("SEC_REIT_SIC_REFRESH")
        .ok()
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));
    let client = reqwest::Client::new();
    let mut entities = HashMap::new();
    for page in 0..100 {
        let start = page * 100;
        let path = cache_dir.join(format!("{start}.html"));
        let bytes = if cache_is_fresh(&path, 7 * 24 * 60 * 60) && !refresh {
            fs::read(&path)?
        } else {
            let separator = if source_url.contains('?') { '&' } else { '?' };
            let url = format!("{source_url}{separator}start={start}");
            let response = client
                .get(url)
                .header(reqwest::header::USER_AGENT, user_agent)
                .send()
                .await
                .map_err(io::Error::other)?
                .error_for_status()
                .map_err(io::Error::other)?;
            let bytes = response.bytes().await.map_err(io::Error::other)?.to_vec();
            if bytes.len() < 1_000 {
                return Err(io::Error::other("SEC SIC response is unexpectedly small"));
            }
            fs::write(&path, &bytes)?;
            actix_web::rt::time::sleep(Duration::from_millis(150)).await;
            bytes
        };
        let html = String::from_utf8_lossy(&bytes);
        let page_entities = extract_sic_entities(&html);
        let has_next = html.contains("value=\"Next100\"");
        entities.extend(page_entities);
        if !has_next {
            break;
        }
    }
    if entities.len() < 100 {
        return Err(io::Error::other(format!(
            "SEC SIC 6798 search returned only {} entities",
            entities.len()
        )));
    }
    Ok(entities)
}

fn cache_is_fresh(path: &Path, max_age_seconds: u64) -> bool {
    path.metadata().ok().is_some_and(|metadata| {
        metadata.len() > 1_000
            && metadata.modified().ok().is_some_and(|modified| {
                modified
                    .elapsed()
                    .ok()
                    .is_some_and(|age| age < Duration::from_secs(max_age_seconds))
            })
    })
}

fn extract_sic_entities(html: &str) -> HashMap<i64, String> {
    let mut entities = HashMap::new();
    for row in html.split("<tr>").skip(1) {
        let Some(cik_start) = row.find("CIK=").map(|index| index + 4) else {
            continue;
        };
        let digits = row[cik_start..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        let Ok(cik) = digits.parse::<i64>() else {
            continue;
        };
        let cells = row.split("<td").collect::<Vec<_>>();
        let Some(name_cell) = cells.get(2) else {
            continue;
        };
        let Some(text_start) = name_cell.find('>').map(|index| index + 1) else {
            continue;
        };
        let Some(text_end) = name_cell[text_start..].find("</td>") else {
            continue;
        };
        let name = decode_html(&name_cell[text_start..text_start + text_end]);
        if !name.is_empty() {
            entities.insert(cik, name);
        }
    }
    entities
}

fn decode_html(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
        .trim()
        .to_owned()
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

fn extract_sic_reits(
    dataset: SecDataset,
    sic_entities: &HashMap<i64, String>,
) -> io::Result<Vec<ListedReit>> {
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
            let cik = row.get(cik_index)?.as_i64()?;
            if !sic_entities.contains_key(&cik) {
                return None;
            }
            let name = row.get(name_index)?.as_str()?.trim();
            let ticker = row.get(ticker_index)?.as_str()?.trim();
            let exchange = row.get(exchange_index)?.as_str()?.trim();
            if ticker.is_empty() || exchange.is_empty() {
                return None;
            }
            Some(ListedReit {
                cik,
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
                    "classification_basis": "Issuer appears in the SEC SIC 6798 Real Estate Investment Trusts search and has a current SEC ticker/exchange identity",
                    "sic": 6798,
                    "identity_source": SOURCE_PAGE,
                    "quote_coverage": "unavailable"
                }));
        });
        query.push(
            " ON CONFLICT (slug) DO UPDATE SET name=EXCLUDED.name,symbol=EXCLUDED.symbol,exchange=EXCLUDED.exchange,provider_id=EXCLUDED.provider_id,source_url=EXCLUDED.source_url,valuation_method=CASE WHEN investment_instruments.status='paper_tradeable' THEN investment_instruments.valuation_method ELSE EXCLUDED.valuation_method END,metadata=investment_instruments.metadata || EXCLUDED.metadata,updated_at=NOW()",
        );
        query.build().execute(&mut **tx).await?;
    }
    Ok(())
}

async fn restore_quote_metadata(
    tx: &mut Transaction<'_, Postgres>,
    provider_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        WITH latest AS (
            SELECT DISTINCT ON (observation.instrument_id)
                observation.instrument_id, observation.methodology, observation.metadata
            FROM instrument_observations AS observation
            WHERE observation.metadata ? 'quote_provider'
            ORDER BY observation.instrument_id, observation.observed_on DESC
        )
        UPDATE investment_instruments AS instrument
        SET status='paper_tradeable',
            valuation_method=latest.methodology,
            metadata=instrument.metadata || jsonb_build_object(
                'quote_provider', latest.metadata->>'quote_provider',
                'quote_license_scope', latest.metadata->>'license_scope',
                'quote_history', 'compact_daily',
                'quote_coverage', 'source_backed'
            ),
            updated_at=NOW()
        FROM latest
        WHERE latest.instrument_id=instrument.id
          AND instrument.provider_id=$1 AND instrument.real_money_enabled=FALSE
        "#,
    )
    .bind(provider_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_exchange_listings_to_sec_sic_6798_entities() {
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
        let entities = HashMap::from([(42, "EXAMPLE REIT INC".to_owned())]);
        let rows = extract_sic_reits(dataset, &entities).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ticker, "REAL");
        assert_eq!(rows[0].exchange, "NYSE");
    }

    #[test]
    fn extracts_ciks_and_names_from_sec_sic_results() {
        let html = r#"<table><tr><th>CIK</th></tr><tr>
          <td valign="top"><a href="/cgi-bin/browse-edgar?action=getcompany&amp;CIK=0001700461&amp;owner=include">0001700461</a></td>
          <td scope="row">1st stREIT Office Inc.</td><td>CA</td></tr></table>"#;
        let entities = extract_sic_entities(html);
        assert_eq!(
            entities.get(&1_700_461).map(String::as_str),
            Some("1st stREIT Office Inc.")
        );
    }
}
