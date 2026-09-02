use std::{env, error::Error};

use landex_api::{
    config::Config,
    ingestion::{CountriesDevProvider, IngestionService},
    state::AppState,
};

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;
    let state = AppState::initialize(&config).await?;
    let cached_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_locations pl JOIN providers p ON p.id=pl.provider_id WHERE p.slug='countries-dev'").fetch_one(&state.database).await?;
    let refresh = env::var("COUNTRIES_DEV_REFRESH")
        .ok()
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));
    let ingested = if cached_count >= 200 && !refresh {
        cached_count as usize
    } else {
        IngestionService::new(state.database.clone())
            .run(&CountriesDevProvider::new()?, 1)
            .await?
            .locations
    };
    let coverage_rows = sqlx::query(r#"
        INSERT INTO country_coverage(country_code, country_name, coverage_depth, provider_slugs, methodology)
        SELECT DISTINCT ON (country_code) country_code, name, 'planned', '["countries-dev"]'::jsonb,
               'Geographic reference coverage only; market, history, listings, and offerings require separate verified sources.'
        FROM locations WHERE kind='country'
        ORDER BY country_code, updated_at DESC
        ON CONFLICT (country_code) DO UPDATE SET
            country_name=EXCLUDED.country_name,
            provider_slugs=(SELECT jsonb_agg(DISTINCT value) FROM jsonb_array_elements(country_coverage.provider_slugs || EXCLUDED.provider_slugs)),
            updated_at=NOW()
    "#).execute(&state.database).await?.rows_affected();
    println!(
        "ingested {} country references; synchronized {} coverage rows",
        ingested, coverage_rows
    );
    Ok(())
}
