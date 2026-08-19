use actix_web::{HttpResponse, post, web};

use crate::{error::ApiError, investment::InvestmentInputs};

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
