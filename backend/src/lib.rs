pub mod auth;
pub mod config;
pub mod domain;
pub mod error;
pub mod ingestion;
pub mod investment;
pub mod market;
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
            .service(routes::investment::analyze_investment)
            .service(routes::locations::list_locations)
            .service(routes::locations::get_location)
            .service(routes::properties::list_properties)
            .service(routes::properties::get_property)
            .service(routes::properties::get_property_history)
            .service(routes::auth::register)
            .service(routes::auth::login)
            .service(routes::auth::refresh)
            .service(routes::auth::logout)
            .service(routes::auth::me),
    );
}

pub fn app_data(state: AppState) -> web::Data<AppState> {
    web::Data::new(state)
}
