use sqlx::{PgPool, postgres::PgPoolOptions};

use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub database: PgPool,
}

impl AppState {
    pub fn new(config: &Config) -> Result<Self, sqlx::Error> {
        let database = PgPoolOptions::new()
            .max_connections(config.database_max_connections)
            .connect_lazy(&config.database_url)?;

        Ok(Self { database })
    }
}
