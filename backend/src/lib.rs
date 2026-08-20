pub mod alerts;
pub mod auth;
pub mod config;
pub mod domain;
pub mod error;
pub mod fx;
pub mod ingestion;
pub mod investment;
pub mod location_intelligence;
pub mod market;
pub mod paper;
pub mod repository;
pub mod routes;
pub mod scoring;
pub mod state;

use actix_web::web;
use state::AppState;

pub fn configure_api(config: &mut web::ServiceConfig) {
    config.service(
        web::scope("/api/v1")
            .service(routes::health::health)
            .service(routes::health::readiness)
            .service(routes::fx::get_rate)
            .service(routes::fx::convert)
            .service(routes::markets::list_markets)
            .service(routes::markets::get_market)
            .service(routes::markets::get_market_score)
            .service(routes::markets::get_market_score_history)
            .service(routes::notifications::list_notifications)
            .service(routes::notifications::mark_notification_read)
            .service(routes::notifications::list_alert_rules)
            .service(routes::notifications::create_alert_rule)
            .service(routes::notifications::update_alert_rule)
            .service(routes::notifications::delete_alert_rule)
            .service(routes::investment::analyze_investment)
            .service(routes::investment::analyze_shortlet)
            .service(routes::investment::score_property)
            .service(routes::investment::simulate_scenarios)
            .service(routes::locations::list_locations)
            .service(routes::locations::get_location)
            .service(routes::properties::list_properties)
            .service(routes::properties::get_property)
            .service(routes::properties::get_property_history)
            .service(routes::properties::get_property_location_intelligence)
            .service(routes::properties::get_property_score)
            .service(routes::properties::get_property_score_history)
            .service(routes::auth::register)
            .service(routes::auth::login)
            .service(routes::auth::refresh)
            .service(routes::auth::logout)
            .service(routes::auth::me)
            .service(routes::watchlists::list_watchlists)
            .service(routes::watchlists::create_watchlist)
            .service(routes::watchlists::get_watchlist)
            .service(routes::watchlists::add_watchlist_item)
            .service(routes::watchlists::remove_watchlist_item)
            .service(routes::paper_accounts::list_paper_accounts)
            .service(routes::paper_accounts::create_paper_account)
            .service(routes::paper_accounts::get_paper_account)
            .service(routes::paper_accounts::get_paper_account_performance)
            .service(routes::paper_accounts::get_paper_account_allocation)
            .service(routes::paper_accounts::get_paper_account_performance_history)
            .service(routes::paper_accounts::list_paper_account_trades)
            .service(routes::paper_accounts::execute_paper_order),
    );
}

pub fn app_data(state: AppState) -> web::Data<AppState> {
    web::Data::new(state)
}
