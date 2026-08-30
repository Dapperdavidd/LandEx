use std::{
    env,
    error::Error,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use landex_api::{
    config::Config,
    historical::{
        hmlr::PricePaidTransaction,
        repository::{HistoricalImportRepository, ImportRun},
    },
    state::AppState,
};
use reqwest::{Client, StatusCode, header};

const MAX_BATCH_SIZE: usize = 3_000;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let source_url = required("HMLR_PRICE_PAID_DATA_URL")?;
    let dataset_key = env::var("HMLR_DATASET_KEY").unwrap_or_else(|_| "complete-history".into());
    let cache_path = PathBuf::from(
        env::var("HMLR_CACHE_PATH").unwrap_or_else(|_| ".data/hmlr/price-paid-complete.csv".into()),
    );
    let batch_size = env::var("HMLR_BATCH_SIZE")
        .ok()
        .map_or(Ok(2_000), |value| value.parse::<usize>())?;
    if batch_size == 0 || batch_size > MAX_BATCH_SIZE {
        return Err(format!("HMLR_BATCH_SIZE must be between 1 and {MAX_BATCH_SIZE}").into());
    }

    let state = AppState::initialize(&Config::from_env()?).await?;
    let repository = HistoricalImportRepository::new(state.database);
    let run = repository.begin("hmlr", &dataset_key, &source_url).await?;
    if run.status == "completed" {
        println!("HMLR dataset {dataset_key} is already complete; no network request made");
        return Ok(());
    }

    if let Err(error) = execute(
        &repository,
        &run,
        &dataset_key,
        &source_url,
        &cache_path,
        batch_size,
    )
    .await
    {
        repository.fail(run.id, &error.to_string()).await?;
        return Err(error);
    }
    repository.complete(run.id).await?;
    println!("HMLR dataset {dataset_key} imported successfully");
    Ok(())
}

async fn execute(
    repository: &HistoricalImportRepository,
    run: &ImportRun,
    dataset_key: &str,
    source_url: &str,
    cache_path: &Path,
    batch_size: usize,
) -> Result<(), Box<dyn Error>> {
    let bytes = download_once(repository, run, source_url, cache_path).await?;
    import_cached(repository, run, dataset_key, cache_path, batch_size, bytes).await
}

async fn download_once(
    repository: &HistoricalImportRepository,
    run: &ImportRun,
    source_url: &str,
    cache_path: &Path,
) -> Result<i64, Box<dyn Error>> {
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let existing = fs::metadata(cache_path)
        .map(|value| value.len())
        .unwrap_or(0);
    let client = Client::builder()
        .user_agent("LandEX historical data importer")
        .build()?;
    let mut request = client.get(source_url);
    if existing > 0 {
        request = request.header(header::RANGE, format!("bytes={existing}-"));
    }
    let mut response = request.send().await?;

    if response.status() == StatusCode::RANGE_NOT_SATISFIABLE
        && completed_range(response.headers(), existing)
    {
        repository
            .checkpoint(
                run.id,
                "import",
                run.checkpoint,
                run.rows_processed,
                existing as i64,
            )
            .await?;
        return Ok(existing as i64);
    }
    if existing > 0 && response.status() != StatusCode::PARTIAL_CONTENT {
        return Err("the source did not honor the resume request; the cache was preserved to prevent a duplicate multi-gigabyte download".into());
    }
    let expected_size = expected_total_size(response.headers(), response.status(), existing);
    response = response.error_for_status()?;

    let mut file = if existing > 0 {
        OpenOptions::new().append(true).open(cache_path)?
    } else {
        File::create(cache_path)?
    };
    let mut downloaded = existing;
    let mut next_checkpoint = existing.saturating_add(16 * 1024 * 1024);
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
        if downloaded >= next_checkpoint {
            repository
                .checkpoint(
                    run.id,
                    "download",
                    run.checkpoint,
                    run.rows_processed,
                    downloaded as i64,
                )
                .await?;
            println!("downloaded {} MiB", downloaded / 1024 / 1024);
            next_checkpoint = downloaded.saturating_add(16 * 1024 * 1024);
        }
    }
    file.sync_all()?;
    if let Some(expected_size) = expected_size
        && downloaded != expected_size
    {
        return Err(format!(
            "download ended at {downloaded} bytes but the source advertised {expected_size}; rerun to resume"
        )
        .into());
    }
    repository
        .checkpoint(
            run.id,
            "import",
            run.checkpoint,
            run.rows_processed,
            downloaded as i64,
        )
        .await?;
    Ok(downloaded as i64)
}

async fn import_cached(
    repository: &HistoricalImportRepository,
    run: &ImportRun,
    dataset_key: &str,
    cache_path: &Path,
    batch_size: usize,
    bytes_downloaded: i64,
) -> Result<(), Box<dyn Error>> {
    let file = File::open(cache_path)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(file);
    let mut batch = Vec::with_capacity(batch_size);
    let mut processed = run.checkpoint;
    for (index, record) in reader.records().enumerate() {
        let row_number = index as i64;
        if row_number < run.checkpoint {
            continue;
        }
        batch.push(PricePaidTransaction::from_record(&record?)?);
        if batch.len() == batch_size {
            processed += batch.len() as i64;
            repository
                .import_hmlr_batch(run.id, dataset_key, &batch, processed, bytes_downloaded)
                .await?;
            batch.clear();
            println!("processed {processed} rows");
        }
    }
    if !batch.is_empty() {
        processed += batch.len() as i64;
        repository
            .import_hmlr_batch(run.id, dataset_key, &batch, processed, bytes_downloaded)
            .await?;
    }
    Ok(())
}

fn completed_range(headers: &header::HeaderMap, existing: u64) -> bool {
    headers
        .get(header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes */"))
        .and_then(|value| value.parse::<u64>().ok())
        == Some(existing)
}

fn expected_total_size(
    headers: &header::HeaderMap,
    status: StatusCode,
    existing: u64,
) -> Option<u64> {
    if status == StatusCode::PARTIAL_CONTENT {
        return headers
            .get(header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.rsplit_once('/'))
            .and_then(|(_, total)| total.parse().ok());
    }
    headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .map(|length| existing + length)
}

fn required(key: &'static str) -> Result<String, Box<dyn Error>> {
    let value = env::var(key)?;
    if value.trim().is_empty() {
        return Err(format!("{key} cannot be empty").into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_a_fully_downloaded_range_response() {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_RANGE, "bytes */5300".parse().unwrap());
        assert!(completed_range(&headers, 5300));
        assert!(!completed_range(&headers, 5299));
    }

    #[test]
    fn reads_the_total_size_from_a_partial_response() {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::CONTENT_RANGE,
            "bytes 1000-5299/5300".parse().unwrap(),
        );
        assert_eq!(
            expected_total_size(&headers, StatusCode::PARTIAL_CONTENT, 1000),
            Some(5300)
        );
    }
}
