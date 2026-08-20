use actix_web::{HttpResponse, post, web};

use crate::{
    error::ApiError,
    investment::{
        InvestmentInputs, PropertyScoreInputs, ScenarioSimulationRequest, ShortletInvestmentInputs,
    },
};

#[post("/analysis/investment")]
pub async fn analyze_investment(
    inputs: web::Json<InvestmentInputs>,
) -> Result<HttpResponse, ApiError> {
    let analysis = inputs
        .into_inner()
        .calculate()
        .map_err(|error| ApiError::InvalidRequest(error.to_string()))?;

    Ok(HttpResponse::Ok().json(analysis))
}

#[post("/analysis/shortlet")]
pub async fn analyze_shortlet(
    inputs: web::Json<ShortletInvestmentInputs>,
) -> Result<HttpResponse, ApiError> {
    let analysis = inputs
        .into_inner()
        .calculate()
        .map_err(|error| ApiError::InvalidRequest(error.to_string()))?;

    Ok(HttpResponse::Ok().json(analysis))
}

#[post("/analysis/property-score")]
pub async fn score_property(
    inputs: web::Json<PropertyScoreInputs>,
) -> Result<HttpResponse, ApiError> {
    let score = inputs
        .into_inner()
        .calculate()
        .map_err(|error| ApiError::InvalidRequest(error.to_string()))?;
    Ok(HttpResponse::Ok().json(score))
}

#[post("/analysis/scenarios")]
pub async fn simulate_scenarios(
    request: web::Json<ScenarioSimulationRequest>,
) -> Result<HttpResponse, ApiError> {
    let simulation = request
        .into_inner()
        .calculate()
        .map_err(|error| ApiError::InvalidRequest(error.to_string()))?;
    Ok(HttpResponse::Ok().json(simulation))
}
