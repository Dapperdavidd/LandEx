pub mod config;
pub mod domain;
pub mod error;
pub mod ingestion;
pub mod repository;
pub mod routes;
pub mod state;

use actix_web::web;
use state::AppState;

pub fn configure_api(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/api/v1")
            .service(routes::health::health)
            .service(routes::health::readiness)
            .service(routes::markets::list_markets)
            .service(routes::markets::get_market)
            .service(routes::properties::list_properties)
            .service(routes::properties::get_property),
    );
}

pub fn app_data(state: AppState) -> web::Data<AppState> {
    web::Data::new(state)
}
