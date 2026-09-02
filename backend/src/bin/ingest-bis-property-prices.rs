use std::{collections::HashMap, env, fs, io, path::Path, process::Command};

use chrono::NaiveDate;
use landex_api::{config::Config, state::AppState};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use sqlx::{Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

const DEFAULT_URL: &str = "https://data.bis.org/static/bulk/WS_SPP_csv_flat.zip";
const SOURCE_PAGE: &str = "https://data.bis.org/bulkdownload";

#[derive(Debug, Deserialize)]
struct BisRow {
    #[serde(rename = "REF_AREA:Reference area")]
    area: String,
    #[serde(rename = "VALUE:Value")]
    value_type: String,
    #[serde(rename = "UNIT_MEASURE:Unit of measure")]
    unit: String,
    #[serde(rename = "TIME_PERIOD:Time period or range")]
    period: String,
    #[serde(rename = "OBS_VALUE:Observation Value")]
    value: Decimal,
}

#[derive(Clone)]
struct Observation {
    country_code: String,
    observed_on: NaiveDate,
    index: Decimal,
    annual_change: Option<Decimal>,
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();
    let config = Config::from_env().map_err(io::Error::other)?;
    let state = AppState::initialize(&config)
        .await
        .map_err(io::Error::other)?;
    let source_url = env::var("BIS_PROPERTY_PRICES_URL").unwrap_or_else(|_| DEFAULT_URL.into());
    let zip_path = env::var("BIS_PROPERTY_PRICES_ZIP_PATH")
        .unwrap_or_else(|_| ".data/bis/selected-property-prices.zip".into());
    let csv_path = env::var("BIS_PROPERTY_PRICES_CSV_PATH")
        .unwrap_or_else(|_| ".data/bis/WS_SPP_csv_flat.csv".into());
    ensure_csv(&source_url, &zip_path, &csv_path).await?;

    let mut countries = HashMap::new();
    let mut indices = HashMap::new();
    let mut changes = HashMap::new();
    for result in csv::Reader::from_path(&csv_path)?.deserialize::<BisRow>() {
        let row = result.map_err(io::Error::other)?;
        let Some((code, name)) = parse_area(&row.area) else {
            continue;
        };
        if row.value_type != "N: Nominal" {
            continue;
        }
        let Some(date) = parse_quarter(&row.period) else {
            continue;
        };
        countries.insert(code.clone(), name);
        let key = (code, date);
        if row.unit.starts_with("628:") {
            indices.insert(key, row.value);
        } else if row.unit.starts_with("771:") {
            changes.insert(key, row.value);
        }
    }
    let observations = indices
        .into_iter()
        .map(|((country_code, observed_on), index)| {
            let annual_change = changes.get(&(country_code.clone(), observed_on)).copied();
            Observation {
                country_code,
                observed_on,
                index,
                annual_change,
            }
        })
        .collect::<Vec<_>>();
    if countries.len() < 50 || observations.len() < 5_000 {
        return Err(io::Error::other(format!(
            "BIS file appears incomplete: {} countries, {} observations",
            countries.len(),
            observations.len()
        )));
    }

    let mut tx = state.database.begin().await.map_err(io::Error::other)?;
    let provider_id = upsert_provider(&mut tx).await.map_err(io::Error::other)?;
    let instruments = prepare_instruments(&mut tx, provider_id, &countries)
        .await
        .map_err(io::Error::other)?;
    for chunk in observations.chunks(500) {
        upsert_observations(&mut tx, chunk, &instruments)
            .await
            .map_err(io::Error::other)?;
    }
    upsert_coverage(&mut tx, &countries)
        .await
        .map_err(io::Error::other)?;
    tx.commit().await.map_err(io::Error::other)?;
    println!(
        "stored {} BIS observations across {} countries",
        observations.len(),
        countries.len()
    );
    Ok(())
}

async fn ensure_csv(source_url: &str, zip_path: &str, csv_path: &str) -> io::Result<()> {
    if Path::new(csv_path)
        .metadata()
        .is_ok_and(|m| m.len() > 1_000_000)
    {
        return Ok(());
    }
    if let Some(parent) = Path::new(zip_path).parent() {
        fs::create_dir_all(parent)?;
    }
    if !Path::new(zip_path)
        .metadata()
        .is_ok_and(|m| m.len() > 100_000)
    {
        let bytes = reqwest::get(source_url)
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .bytes()
            .await
            .map_err(io::Error::other)?;
        if bytes.len() < 100_000 {
            return Err(io::Error::other("BIS bulk response is unexpectedly small"));
        }
        fs::write(zip_path, bytes)?;
    }
    if let Some(parent) = Path::new(csv_path).parent() {
        fs::create_dir_all(parent)?;
    }
    let output = Command::new("unzip").args(["-p", zip_path]).output()?;
    if !output.status.success() || output.stdout.len() < 1_000_000 {
        return Err(io::Error::other("could not extract a complete BIS CSV"));
    }
    fs::write(csv_path, output.stdout)?;
    Ok(())
}

fn parse_area(value: &str) -> Option<(String, String)> {
    let (code, name) = value.split_once(':')?;
    let code = code.trim();
    if code.len() != 2
        || !code.bytes().all(|b| b.is_ascii_alphabetic())
        || matches!(code, "XW" | "XM")
    {
        return None;
    }
    Some((code.to_ascii_uppercase(), name.trim().to_owned()))
}

fn parse_quarter(value: &str) -> Option<NaiveDate> {
    let (year, quarter) = value.split_once("-Q")?;
    let year: i32 = year.parse().ok()?;
    let month: u32 = quarter.parse::<u32>().ok()?.checked_mul(3)?;
    let first_next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?
    };
    first_next.pred_opt()
}

async fn upsert_provider(tx: &mut Transaction<'_, Postgres>) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar("INSERT INTO providers (slug,name) VALUES ('bis-property-prices','BIS Residential Property Prices') ON CONFLICT (slug) DO UPDATE SET updated_at=NOW() RETURNING id").fetch_one(&mut **tx).await
}

async fn prepare_instruments(
    tx: &mut Transaction<'_, Postgres>,
    provider_id: Uuid,
    countries: &HashMap<String, String>,
) -> Result<HashMap<String, Uuid>, sqlx::Error> {
    let mut codes = countries.keys().cloned().collect::<Vec<_>>();
    codes.sort();
    let names = codes
        .iter()
        .map(|c| countries[c].clone())
        .collect::<Vec<_>>();
    sqlx::query(r#"INSERT INTO locations (kind,name,normalized_name,country_code)
        SELECT 'country', source.name, LOWER(source.name), source.code FROM UNNEST($1::text[],$2::text[]) source(code,name)
        ON CONFLICT (parent_id,kind,normalized_name,country_code) DO UPDATE SET name=EXCLUDED.name,updated_at=NOW()"#)
        .bind(&codes).bind(&names).execute(&mut **tx).await?;
    sqlx::query(r#"INSERT INTO investment_instruments (slug,name,instrument_kind,status,country_code,currency,location_id,provider_id,source_url,valuation_method,liquidity_class,metadata)
        SELECT 'bis-'||LOWER(source.code)||'-residential-price-index', source.name||' Residential Price Index',
        'market_proxy','paper_tradeable',source.code,'XXX',location.id,$3,$4,
        'BIS selected nominal residential property price index; paper performance follows quarterly index movement',
        'index_proxy',jsonb_build_object('value_kind','index','frequency','quarterly','country_name',source.name)
        FROM UNNEST($1::text[],$2::text[]) source(code,name) JOIN locations location ON location.kind='country' AND location.country_code=source.code
        ON CONFLICT (slug) DO UPDATE SET name=EXCLUDED.name,location_id=EXCLUDED.location_id,provider_id=EXCLUDED.provider_id,source_url=EXCLUDED.source_url,valuation_method=EXCLUDED.valuation_method,metadata=EXCLUDED.metadata,updated_at=NOW()"#)
        .bind(&codes).bind(&names).bind(provider_id).bind(SOURCE_PAGE).execute(&mut **tx).await?;
    let pairs = sqlx::query_as::<_,(Uuid,String)>("SELECT id,country_code FROM investment_instruments WHERE provider_id=$1 AND instrument_kind='market_proxy'").bind(provider_id).fetch_all(&mut **tx).await?;
    Ok(pairs.into_iter().map(|(id, code)| (code, id)).collect())
}

async fn upsert_observations(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[Observation],
    instruments: &HashMap<String, Uuid>,
) -> Result<(), sqlx::Error> {
    let mut query = QueryBuilder::<Postgres>::new(
        "INSERT INTO instrument_observations (instrument_id,observed_on,value,currency,annual_change_percent,source_url,methodology,metadata) ",
    );
    query.push_values(rows,|mut values,row| { values.push_bind(instruments[&row.country_code]).push_bind(row.observed_on).push_bind(row.index).push_bind("XXX").push_bind(row.annual_change).push_bind(SOURCE_PAGE).push_bind("BIS selected nominal residential property price index and official year-on-year change").push_bind(json!({"value_kind":"index","frequency":"quarterly"})); });
    query.push(" ON CONFLICT (instrument_id,observed_on) DO UPDATE SET value=EXCLUDED.value,annual_change_percent=EXCLUDED.annual_change_percent,source_url=EXCLUDED.source_url,methodology=EXCLUDED.methodology,metadata=EXCLUDED.metadata");
    query.build().execute(&mut **tx).await?;
    Ok(())
}

async fn upsert_coverage(
    tx: &mut Transaction<'_, Postgres>,
    countries: &HashMap<String, String>,
) -> Result<(), sqlx::Error> {
    let mut codes = countries.keys().cloned().collect::<Vec<_>>();
    codes.sort();
    let names = codes
        .iter()
        .map(|c| countries[c].clone())
        .collect::<Vec<_>>();
    sqlx::query(r#"INSERT INTO country_coverage (country_code,country_name,coverage_depth,has_market_data,has_historical_data,provider_slugs,methodology,latest_observation_on)
        SELECT source.code,source.name,'basic',TRUE,TRUE,'["bis-property-prices"]'::jsonb,'BIS selected nationwide residential property price index',MAX(io.observed_on)
        FROM UNNEST($1::text[],$2::text[]) source(code,name) JOIN investment_instruments i ON i.country_code=source.code AND i.provider_id=(SELECT id FROM providers WHERE slug='bis-property-prices') JOIN instrument_observations io ON io.instrument_id=i.id GROUP BY source.code,source.name
        ON CONFLICT(country_code) DO UPDATE SET country_name=EXCLUDED.country_name,coverage_depth=CASE WHEN country_coverage.coverage_depth IN ('standard','deep') THEN country_coverage.coverage_depth ELSE 'basic' END,has_market_data=TRUE,has_historical_data=TRUE,provider_slugs=(SELECT jsonb_agg(DISTINCT value) FROM jsonb_array_elements(country_coverage.provider_slugs||EXCLUDED.provider_slugs)),methodology=COALESCE(country_coverage.methodology||'; ','')||EXCLUDED.methodology,latest_observation_on=GREATEST(country_coverage.latest_observation_on,EXCLUDED.latest_observation_on),updated_at=NOW()"#).bind(&codes).bind(&names).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    #[test]
    fn parses_country_and_quarter() {
        assert_eq!(
            parse_area("NG: Nigeria"),
            Some(("NG".into(), "Nigeria".into()))
        );
        assert_eq!(parse_area("XW: World"), None);
        let date = parse_quarter("2026-Q1").unwrap();
        assert_eq!((date.year(), date.month(), date.day()), (2026, 3, 31));
    }
}
