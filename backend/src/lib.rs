pub mod config;
pub mod error;
pub mod routes;
pub mod state;

use actix_web::web;
use state::AppState;

pub fn configure_api(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/api/v1")
            .service(routes::health::health)
            .service(routes::health::readiness),
    );
}

pub fn app_data(state: AppState) -> web::Data<AppState> {
    web::Data::new(state)
}
