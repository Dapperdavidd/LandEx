use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use super::hmlr::PricePaidTransaction;

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

    pub async fn import_hmlr_batch(
        &self,
        run_id: Uuid,
        source_dataset: &str,
        rows: &[PricePaidTransaction],
        checkpoint: i64,
        bytes_downloaded: i64,
    ) -> Result<(), sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let deleted: Vec<Uuid> = rows
            .iter()
            .filter(|row| row.record_status == 'D')
            .map(|row| row.transaction_id)
            .collect();
        if !deleted.is_empty() {
            sqlx::query("DELETE FROM hmlr_price_paid_transactions WHERE transaction_id = ANY($1)")
                .bind(&deleted)
                .execute(&mut *transaction)
                .await?;
        }

        let upserts: Vec<&PricePaidTransaction> =
            rows.iter().filter(|row| row.record_status != 'D').collect();
        if !upserts.is_empty() {
            let mut query = QueryBuilder::<Postgres>::new(
                "INSERT INTO hmlr_price_paid_transactions (transaction_id, price, transferred_on, postcode, property_type, new_build, tenure, paon, saon, street, locality, town_city, district, county, ppd_category, record_status, source_dataset) ",
            );
            query.push_values(upserts, |mut values, row| {
                values
                    .push_bind(row.transaction_id)
                    .push_bind(row.price)
                    .push_bind(row.transferred_on)
                    .push_bind(&row.postcode)
                    .push_bind(row.property_type.to_string())
                    .push_bind(row.new_build)
                    .push_bind(row.tenure.to_string())
                    .push_bind(&row.paon)
                    .push_bind(&row.saon)
                    .push_bind(&row.street)
                    .push_bind(&row.locality)
                    .push_bind(&row.town_city)
                    .push_bind(&row.district)
                    .push_bind(&row.county)
                    .push_bind(row.ppd_category.to_string())
                    .push_bind(row.record_status.to_string())
                    .push_bind(source_dataset);
            });
            query.push(
                " ON CONFLICT (transaction_id) DO UPDATE SET price=EXCLUDED.price, transferred_on=EXCLUDED.transferred_on, postcode=EXCLUDED.postcode, property_type=EXCLUDED.property_type, new_build=EXCLUDED.new_build, tenure=EXCLUDED.tenure, paon=EXCLUDED.paon, saon=EXCLUDED.saon, street=EXCLUDED.street, locality=EXCLUDED.locality, town_city=EXCLUDED.town_city, district=EXCLUDED.district, county=EXCLUDED.county, ppd_category=EXCLUDED.ppd_category, record_status=EXCLUDED.record_status, source_dataset=EXCLUDED.source_dataset, imported_at=NOW()",
            );
            query.build().execute(&mut *transaction).await?;
        }

        sqlx::query(
            "UPDATE data_import_runs SET phase='import', checkpoint=$2, rows_processed=$2, bytes_downloaded=$3, updated_at=NOW() WHERE id=$1 AND status='running'",
        )
        .bind(run_id)
        .bind(checkpoint)
        .bind(bytes_downloaded)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await
    }

    pub async fn complete(&self, id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE data_import_runs SET status='completed', phase='complete', completed_at=NOW(), updated_at=NOW(), error_message=NULL WHERE id=$1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn fail(&self, id: Uuid, message: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE data_import_runs SET status='failed', error_message=$2, updated_at=NOW() WHERE id=$1",
        )
        .bind(id)
        .bind(message)
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
