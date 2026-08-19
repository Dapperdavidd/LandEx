use sqlx::{PgPool, migrate::MigrateError, postgres::PgPoolOptions};
use thiserror::Error;

use crate::config::Config;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Clone)]
pub struct AppState {
    pub database: PgPool,
}

impl AppState {
    pub async fn initialize(config: &Config) -> Result<Self, StateInitializationError> {
        let database = PgPoolOptions::new()
            .max_connections(config.database_max_connections)
            .connect(&config.database_url)
            .await?;

        MIGRATOR.run(&database).await?;

        Ok(Self { database })
    }
}

#[derive(Debug, Error)]
pub enum StateInitializationError {
    #[error("could not connect to PostgreSQL: {0}")]
    Database(#[from] sqlx::Error),
    #[error("could not apply database migrations: {0}")]
    Migration(#[from] MigrateError),
}
