use std::io;

use actix_web::{App, HttpServer};
use landex_api::{app_data, config::Config, configure_api, state::AppState};
use tracing::info;
use tracing_actix_web::TracingLogger;
use tracing_subscriber::EnvFilter;

#[actix_web::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("landex_api=debug,actix_web=info")),
        )
        .init();

    let config = Config::from_env().map_err(io::Error::other)?;
    let address = config.server_address();
    let state = app_data(AppState::new(&config).map_err(io::Error::other)?);

    info!(?address, "starting LandEX API");

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(TracingLogger::default())
            .configure(configure_api)
    })
    .bind(address)?
    .run()
    .await
}
