use std::{collections::BTreeMap, env, io, time::Duration};

use chrono::NaiveDate;
use landex_api::{config::Config, state::AppState};
use reqwest::Url;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

const DEFAULT_BASE_URL: &str = "https://www.alphavantage.co/query";
const SOURCE_PAGE: &str = "https://www.alphavantage.co/documentation/";
const PROVIDER_SLUG: &str = "alpha-vantage-development-quotes";
const DAILY_REQUEST_LIMIT: i64 = 20;

#[derive(Debug, Deserialize)]
struct DailyResponse {
    #[serde(rename = "Meta Data")]
    metadata: Option<Value>,
    #[serde(rename = "Time Series (Daily)")]
    series: Option<BTreeMap<String, DailyBar>>,
    #[serde(rename = "Error Message")]
    error_message: Option<String>,
    #[serde(rename = "Information")]
    information: Option<String>,
    #[serde(rename = "Note")]
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DailyBar {
    #[serde(rename = "4. close")]
    close: Decimal,
}

#[derive(Debug)]
struct InstrumentTarget {
    id: Uuid,
    symbol: String,
}

#[derive(Debug)]
struct PricePoint {
    observed_on: NaiveDate,
    close: Decimal,
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();
    if !read_bool("ALPHA_VANTAGE_DEVELOPMENT_MODE") {
        return Err(io::Error::other(
            "ALPHA_VANTAGE_DEVELOPMENT_MODE=true is required; the free feed is development-only and must not be treated as production redistribution rights",
        ));
    }
    let api_key = env::var("ALPHA_VANTAGE_API_KEY")
        .map_err(|_| io::Error::other("ALPHA_VANTAGE_API_KEY is required"))?;
    if api_key.trim().is_empty() {
        return Err(io::Error::other("ALPHA_VANTAGE_API_KEY cannot be empty"));
    }
    let base_url = env::var("ALPHA_VANTAGE_API_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.into());
    let base_url = Url::parse(&base_url).map_err(io::Error::other)?;
    let config = Config::from_env().map_err(io::Error::other)?;
    let state = AppState::initialize(&config)
        .await
        .map_err(io::Error::other)?;
    let provider_id = upsert_provider(&state.database)
        .await
        .map_err(io::Error::other)?;
    let remaining = remaining_requests(&state.database, provider_id)
        .await
        .map_err(io::Error::other)?;
    if remaining == 0 {
        return Err(io::Error::other(
            "Alpha Vantage development guard reached 20 attempts in the last 24 hours",
        ));
    }
    let targets = load_targets(&state.database, remaining)
        .await
        .map_err(io::Error::other)?;
    if targets.is_empty() {
        println!("all supported listed REIT quotes are already fresh; used 0 requests");
        return Ok(());
    }

    let client = reqwest::Client::new();
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    for (index, target) in targets.into_iter().enumerate() {
        if index > 0 {
            actix_web::rt::time::sleep(Duration::from_millis(1_100)).await;
        }
        let attempt_id = reserve_request(&state.database, provider_id, &target.symbol)
            .await
            .map_err(io::Error::other)?;
        match fetch_series(&client, &base_url, &api_key, &target.symbol).await {
            Ok(points) => {
                persist_series(&state.database, &target, &points)
                    .await
                    .map_err(io::Error::other)?;
                record_outcome(&state.database, attempt_id, "succeeded")
                    .await
                    .map_err(io::Error::other)?;
                succeeded += 1;
            }
            Err(error) => {
                record_outcome(&state.database, attempt_id, "failed")
                    .await
                    .map_err(io::Error::other)?;
                eprintln!("{}: {error}", target.symbol);
                failed += 1;
            }
        }
    }
    println!("refreshed {succeeded} listed REIT quote series; {failed} failed");
    Ok(())
}

fn read_bool(key: &str) -> bool {
    env::var(key)
        .ok()
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

async fn upsert_provider(pool: &PgPool) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO providers (slug,name) VALUES ($1,'Alpha Vantage Development Quotes') ON CONFLICT (slug) DO UPDATE SET updated_at=NOW() RETURNING id",
    )
    .bind(PROVIDER_SLUG)
    .fetch_one(pool)
    .await
}

async fn remaining_requests(pool: &PgPool, provider_id: Uuid) -> Result<i64, sqlx::Error> {
    let attempts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM provider_request_attempts WHERE provider_id=$1 AND requested_at >= NOW() - INTERVAL '24 hours'",
    )
    .bind(provider_id)
    .fetch_one(pool)
    .await?;
    Ok((DAILY_REQUEST_LIMIT - attempts).max(0))
}

async fn load_targets(pool: &PgPool, limit: i64) -> Result<Vec<InstrumentTarget>, sqlx::Error> {
    sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT i.id, i.symbol
        FROM investment_instruments i
        JOIN providers identity_provider ON identity_provider.id=i.provider_id
        LEFT JOIN LATERAL (
            SELECT MAX(observed_on) AS latest_on
            FROM instrument_observations WHERE instrument_id=i.id
        ) latest ON TRUE
        WHERE identity_provider.slug='sec-edgar-listed-reits'
          AND i.instrument_kind='listed_security'
          AND i.exchange IN ('NYSE','Nasdaq')
          AND i.symbol IS NOT NULL
          AND i.symbol NOT LIKE '%-%'
          AND (latest.latest_on IS NULL OR latest.latest_on < CURRENT_DATE)
        ORDER BY latest.latest_on NULLS FIRST, i.name, i.symbol
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|(id, symbol)| InstrumentTarget { id, symbol })
            .collect()
    })
}

async fn reserve_request(
    pool: &PgPool,
    provider_id: Uuid,
    symbol: &str,
) -> Result<Uuid, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("provider-request-budget:{PROVIDER_SLUG}"))
        .execute(&mut *tx)
        .await?;
    let remaining = remaining_requests_in_tx(&mut tx, provider_id).await?;
    if remaining == 0 {
        return Err(sqlx::Error::Protocol(
            "Alpha Vantage development request guard reached 20 attempts in 24 hours".into(),
        ));
    }
    let id = sqlx::query_scalar(
        "INSERT INTO provider_request_attempts(provider_id,cursor) VALUES($1,$2) RETURNING id",
    )
    .bind(provider_id)
    .bind(symbol)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

async fn remaining_requests_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    provider_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let attempts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM provider_request_attempts WHERE provider_id=$1 AND requested_at >= NOW() - INTERVAL '24 hours'",
    )
    .bind(provider_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok((DAILY_REQUEST_LIMIT - attempts).max(0))
}

async fn record_outcome(pool: &PgPool, id: Uuid, outcome: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE provider_request_attempts SET outcome=$2 WHERE id=$1")
        .bind(id)
        .bind(outcome)
        .execute(pool)
        .await?;
    Ok(())
}

async fn fetch_series(
    client: &reqwest::Client,
    base_url: &Url,
    api_key: &str,
    symbol: &str,
) -> io::Result<Vec<PricePoint>> {
    let response = client
        .get(base_url.clone())
        .query(&[
            ("function", "TIME_SERIES_DAILY"),
            ("symbol", symbol),
            ("outputsize", "compact"),
            ("apikey", api_key),
        ])
        .send()
        .await
        .map_err(io::Error::other)?
        .error_for_status()
        .map_err(io::Error::other)?
        .json::<DailyResponse>()
        .await
        .map_err(io::Error::other)?;
    parse_series(response)
}

fn parse_series(response: DailyResponse) -> io::Result<Vec<PricePoint>> {
    if let Some(message) = response
        .error_message
        .or(response.information)
        .or(response.note)
    {
        return Err(io::Error::other(message));
    }
    if response.metadata.is_none() {
        return Err(io::Error::other("Alpha Vantage response omitted metadata"));
    }
    let series = response
        .series
        .ok_or_else(|| io::Error::other("Alpha Vantage response omitted daily series"))?;
    let mut points = series
        .into_iter()
        .map(|(date, bar)| {
            let observed_on = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                .map_err(|error| io::Error::other(error.to_string()))?;
            if bar.close <= Decimal::ZERO {
                return Err(io::Error::other("daily close must be positive"));
            }
            Ok(PricePoint {
                observed_on,
                close: bar.close,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    points.sort_by_key(|point| point.observed_on);
    if points.len() < 20 {
        return Err(io::Error::other(format!(
            "daily series contains only {} observations",
            points.len()
        )));
    }
    Ok(points)
}

async fn persist_series(
    pool: &PgPool,
    target: &InstrumentTarget,
    points: &[PricePoint],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for chunk in points.chunks(250) {
        let mut query = QueryBuilder::<Postgres>::new(
            "INSERT INTO instrument_observations(instrument_id,observed_on,value,currency,source_url,methodology,metadata) ",
        );
        query.push_values(chunk, |mut values, point| {
            values
                .push_bind(target.id)
                .push_bind(point.observed_on)
                .push_bind(point.close)
                .push_bind("USD")
                .push_bind(SOURCE_PAGE)
                .push_bind("Alpha Vantage unadjusted end-of-day close; compact development series")
                .push_bind(json!({
                    "value_kind": "unadjusted_close",
                    "frequency": "daily",
                    "quote_provider": PROVIDER_SLUG,
                    "license_scope": "development_noncommercial"
                }));
        });
        query.push(
            " ON CONFLICT(instrument_id,observed_on) DO UPDATE SET value=EXCLUDED.value,currency=EXCLUDED.currency,source_url=EXCLUDED.source_url,methodology=EXCLUDED.methodology,metadata=EXCLUDED.metadata",
        );
        query.build().execute(&mut *tx).await?;
    }
    sqlx::query(
        r#"
        UPDATE investment_instruments
        SET status='paper_tradeable', valuation_method='Alpha Vantage unadjusted end-of-day close for development paper simulation',
            metadata=metadata || jsonb_build_object('quote_provider',$2,'quote_license_scope','development_noncommercial','quote_history','compact_daily'),
            updated_at=NOW()
        WHERE id=$1 AND real_money_enabled=FALSE
        "#,
    )
    .bind(target.id)
    .bind(PROVIDER_SLUG)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_orders_positive_daily_closes() {
        let series = (1..=20)
            .map(|day| {
                (
                    format!("2026-08-{day:02}"),
                    DailyBar {
                        close: Decimal::from(day),
                    },
                )
            })
            .collect();
        let points = parse_series(DailyResponse {
            metadata: Some(json!({"symbol":"TEST"})),
            series: Some(series),
            error_message: None,
            information: None,
            note: None,
        })
        .unwrap();
        assert_eq!(points.len(), 20);
        assert_eq!(points[0].observed_on.to_string(), "2026-08-01");
        assert_eq!(points[19].close, Decimal::from(20));
    }

    #[test]
    fn surfaces_provider_limit_messages() {
        let error = parse_series(DailyResponse {
            metadata: None,
            series: None,
            error_message: None,
            information: Some("daily limit reached".into()),
            note: None,
        })
        .unwrap_err();
        assert!(error.to_string().contains("daily limit"));
    }
}
