use std::{collections::HashMap, env, fs, io, path::Path};

use landex_api::{config::Config, historical::uk_hpi::UkHpiRow, state::AppState};
use serde_json::json;
use sqlx::{Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

const DEFAULT_URL: &str = "https://publicdata.landregistry.gov.uk/market-trend-data/house-price-index-data/UK-HPI-full-file-2026-06.csv";
const SOURCE_PAGE: &str = "https://www.gov.uk/government/statistical-data-sets/uk-house-price-index-data-downloads-june-2026";

#[actix_web::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();
    let config = Config::from_env().map_err(io::Error::other)?;
    let state = AppState::initialize(&config)
        .await
        .map_err(io::Error::other)?;
    let source_url = env::var("UK_HPI_CSV_URL").unwrap_or_else(|_| DEFAULT_URL.into());
    let cache_path =
        env::var("UK_HPI_CACHE_PATH").unwrap_or_else(|_| ".data/hmlr/uk-hpi-full.csv".into());
    ensure_cache(&source_url, &cache_path).await?;

    let file = fs::File::open(&cache_path)?;
    let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(file);
    let mut rows = Vec::new();
    for result in reader.deserialize::<UkHpiRow>() {
        let row = result.map_err(io::Error::other)?;
        row.validate().map_err(io::Error::other)?;
        rows.push(row);
    }
    if rows.len() < 1_000 {
        return Err(io::Error::other(format!(
            "UK HPI file appears incomplete: only {} rows",
            rows.len()
        )));
    }
    let mut tx = state.database.begin().await.map_err(io::Error::other)?;
    let provider_id = upsert_provider(&mut tx).await.map_err(io::Error::other)?;
    let instruments = prepare_instruments(&mut tx, provider_id, &rows)
        .await
        .map_err(io::Error::other)?;
    println!("prepared {} official market instruments", instruments.len());
    for (index, chunk) in rows.chunks(500).enumerate() {
        upsert_observations(&mut tx, chunk, &instruments)
            .await
            .map_err(io::Error::other)?;
        if (index + 1) % 50 == 0 {
            println!(
                "staged {} of {} observations",
                (index + 1) * 500,
                rows.len()
            );
        }
    }
    sqlx::query(
        r#"
        INSERT INTO country_coverage (country_code, country_name, coverage_depth,
            has_market_data, has_historical_data, provider_slugs, methodology,
            latest_observation_on, updated_at)
        SELECT 'GB', 'United Kingdom', 'standard', TRUE, TRUE,
            '["uk-house-price-index"]'::jsonb,
            'Official UK House Price Index monthly average prices and indices',
            MAX(observed_on), NOW()
        FROM instrument_observations io
        JOIN investment_instruments i ON i.id = io.instrument_id
        WHERE i.country_code = 'GB' AND i.instrument_kind = 'market_proxy'
        ON CONFLICT (country_code) DO UPDATE SET
            coverage_depth = EXCLUDED.coverage_depth,
            has_market_data = TRUE, has_historical_data = TRUE,
            provider_slugs = EXCLUDED.provider_slugs,
            methodology = EXCLUDED.methodology,
            latest_observation_on = EXCLUDED.latest_observation_on,
            updated_at = NOW()
    "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(io::Error::other)?;
    tx.commit().await.map_err(io::Error::other)?;
    println!(
        "stored {} official UK HPI observations across {} market instruments",
        rows.len(),
        instruments.len()
    );
    Ok(())
}

async fn ensure_cache(source_url: &str, cache_path: &str) -> io::Result<()> {
    if Path::new(cache_path)
        .metadata()
        .is_ok_and(|m| m.len() > 1_000_000)
    {
        return Ok(());
    }
    if let Some(parent) = Path::new(cache_path).parent() {
        fs::create_dir_all(parent)?;
    }
    let response = reqwest::get(source_url)
        .await
        .map_err(io::Error::other)?
        .error_for_status()
        .map_err(io::Error::other)?;
    let bytes = response.bytes().await.map_err(io::Error::other)?;
    if bytes.len() < 1_000_000 {
        return Err(io::Error::other(
            "UK HPI response is unexpectedly small; refusing to cache a partial file",
        ));
    }
    fs::write(cache_path, bytes)?;
    Ok(())
}

async fn upsert_provider(tx: &mut Transaction<'_, Postgres>) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO providers (slug, name) VALUES ('uk-house-price-index', 'UK House Price Index')
        ON CONFLICT (slug) DO UPDATE SET updated_at = NOW() RETURNING id
    "#,
    )
    .fetch_one(&mut **tx)
    .await
}

async fn prepare_instruments(
    tx: &mut Transaction<'_, Postgres>,
    provider_id: Uuid,
    rows: &[UkHpiRow],
) -> Result<HashMap<String, Uuid>, sqlx::Error> {
    let mut regions = HashMap::new();
    for row in rows {
        regions
            .entry(row.area_code.clone())
            .or_insert_with(|| row.region_name.trim().to_owned());
    }
    let mut area_codes = regions.keys().cloned().collect::<Vec<_>>();
    area_codes.sort();
    let region_names = area_codes
        .iter()
        .map(|code| regions[code].clone())
        .collect::<Vec<_>>();
    sqlx::query(
        r#"
        INSERT INTO locations (kind, name, normalized_name, country_code, administrative_code)
        SELECT 'region', source.name, LOWER(source.name), 'GB', source.area_code
        FROM UNNEST($1::text[], $2::text[]) AS source(area_code, name)
        ON CONFLICT (parent_id, kind, normalized_name, country_code)
        DO UPDATE SET administrative_code = EXCLUDED.administrative_code, updated_at = NOW()
    "#,
    )
    .bind(&area_codes)
    .bind(&region_names)
    .execute(&mut **tx)
    .await?;
    sqlx::query(r#"
        INSERT INTO investment_instruments (slug, name, instrument_kind, status, country_code,
            currency, location_id, provider_id, source_url, valuation_method, liquidity_class, metadata)
        SELECT 'gb-' || LOWER(source.area_code) || '-residential-price-index',
            source.name || ' Residential Market', 'market_proxy', 'paper_tradeable',
            'GB', 'GBP', location.id, $3, $4,
            'Official UK HPI average price; paper valuation follows the published monthly index',
            'index_proxy', jsonb_build_object('area_code', source.area_code, 'official_index_series', TRUE)
        FROM UNNEST($1::text[], $2::text[]) AS source(area_code, name)
        JOIN locations location ON location.country_code='GB'
            AND location.kind='region' AND location.administrative_code=source.area_code
        ON CONFLICT (slug) DO UPDATE SET name=EXCLUDED.name, location_id=EXCLUDED.location_id,
            provider_id=EXCLUDED.provider_id, source_url=EXCLUDED.source_url,
            valuation_method=EXCLUDED.valuation_method, metadata=EXCLUDED.metadata, updated_at=NOW()
    "#).bind(&area_codes).bind(&region_names).bind(provider_id).bind(SOURCE_PAGE).execute(&mut **tx).await?;
    let pairs = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, metadata->>'area_code' FROM investment_instruments
        WHERE provider_id=$1 AND instrument_kind='market_proxy' AND country_code='GB'
            AND metadata ? 'area_code'
    "#,
    )
    .bind(provider_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(pairs.into_iter().map(|(id, code)| (code, id)).collect())
}

async fn upsert_observations(
    tx: &mut Transaction<'_, Postgres>,
    rows: &[UkHpiRow],
    instruments: &HashMap<String, Uuid>,
) -> Result<(), sqlx::Error> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        INSERT INTO instrument_observations (instrument_id, observed_on, value,
            currency, annual_change_percent, source_url, methodology, metadata)
    "#,
    );
    query.push_values(rows, |mut values, row| {
        values
            .push_bind(instruments[&row.area_code])
            .push_bind(row.observed_on)
            .push_bind(row.average_price)
            .push_bind("GBP")
            .push_bind(row.annual_change_percent)
            .push_bind(SOURCE_PAGE)
            .push_bind("Published average residential sale price and 12-month change from UK HPI")
            .push_bind(json!({"index": row.index, "sales_volume": row.sales_volume}));
    });
    query.push(
        r#"
        ON CONFLICT (instrument_id, observed_on) DO UPDATE SET value=EXCLUDED.value,
            annual_change_percent=EXCLUDED.annual_change_percent, source_url=EXCLUDED.source_url,
            methodology=EXCLUDED.methodology, metadata=EXCLUDED.metadata
    "#,
    );
    query.build().execute(&mut **tx).await?;
    Ok(())
}
