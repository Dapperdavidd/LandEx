use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct HistoricalImportRepository {
    pool: PgPool,
}

impl HistoricalImportRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn begin(
        &self,
        source_slug: &str,
        dataset_key: &str,
        source_url: &str,
    ) -> Result<ImportRun, sqlx::Error> {
        sqlx::query_as(
            r#"
            INSERT INTO data_import_runs (source_slug, dataset_key, source_url, status, started_at, error_message)
            VALUES ($1, $2, $3, 'running', NOW(), NULL)
            ON CONFLICT (source_slug, dataset_key) DO UPDATE SET
                source_url = EXCLUDED.source_url,
                status = CASE WHEN data_import_runs.status = 'completed' THEN 'completed' ELSE 'running' END,
                started_at = CASE WHEN data_import_runs.status = 'completed' THEN data_import_runs.started_at ELSE NOW() END,
                error_message = NULL,
                updated_at = NOW()
            RETURNING id, status, phase, checkpoint, rows_processed, bytes_downloaded
            "#,
        )
        .bind(source_slug)
        .bind(dataset_key)
        .bind(source_url)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn checkpoint(
        &self,
        id: Uuid,
        phase: &str,
        checkpoint: i64,
        rows_processed: i64,
        bytes_downloaded: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE data_import_runs SET phase=$2, checkpoint=$3, rows_processed=$4, bytes_downloaded=$5, updated_at=NOW() WHERE id=$1 AND status='running'",
        )
        .bind(id)
        .bind(phase)
        .bind(checkpoint)
        .bind(rows_processed)
        .bind(bytes_downloaded)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct ImportRun {
    pub id: Uuid,
    pub status: String,
    pub phase: String,
    pub checkpoint: i64,
    pub rows_processed: i64,
    pub bytes_downloaded: i64,
}
