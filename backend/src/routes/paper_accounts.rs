use actix_web::{HttpRequest, HttpResponse, get, post, web};
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::error;
use uuid::Uuid;

use crate::{
    error::ApiError,
    repository::instrument_portfolio::InstrumentPortfolioRepository,
    repository::paper_account::{PaperAccountRepository, PaperTradeError},
    routes::auth::authenticate,
    state::AppState,
};

const DEFAULT_DEMO_CASH: Decimal = Decimal::from_parts(100_000, 0, 0, false, 0);

#[derive(Debug, Deserialize)]
pub struct CreatePaperAccountRequest {
    #[serde(default = "default_account_name")]
    name: String,
    #[serde(default = "default_currency")]
    base_currency: String,
    #[serde(default = "default_demo_cash", with = "rust_decimal::serde::str")]
    starting_cash: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct PaperOrderRequest {
    property_id: Uuid,
    side: String,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    amount: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    units: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
pub struct InstrumentPaperOrderRequest {
    instrument_id: Uuid,
    side: String,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    amount: Option<Decimal>,
    #[serde(default, with = "rust_decimal::serde::str_option")]
    units: Option<Decimal>,
}

#[get("/paper-accounts")]
pub async fn list_paper_accounts(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let accounts = PaperAccountRepository::new(state.database.clone())
        .list(user.id)
        .await
        .map_err(database_error)?;
    Ok(HttpResponse::Ok().json(accounts))
}

#[post("/paper-accounts")]
pub async fn create_paper_account(
    state: web::Data<AppState>,
    request: HttpRequest,
    body: web::Json<CreatePaperAccountRequest>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let name = body.name.trim();
    let currency = body.base_currency.trim().to_uppercase();
    validate(name, &currency, body.starting_cash)?;

    let account = PaperAccountRepository::new(state.database.clone())
        .create(user.id, name, &currency, body.starting_cash)
        .await
        .map_err(map_create_error)?;
    Ok(HttpResponse::Created().json(account))
}

#[get("/paper-accounts/{account_id}")]
pub async fn get_paper_account(
    state: web::Data<AppState>,
    request: HttpRequest,
    account_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let account = PaperAccountRepository::new(state.database.clone())
        .detail(user.id, account_id.into_inner())
        .await
        .map_err(database_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(account))
}

#[get("/paper-accounts/{account_id}/performance")]
pub async fn get_paper_account_performance(
    state: web::Data<AppState>,
    request: HttpRequest,
    account_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let performance = PaperAccountRepository::new(state.database.clone())
        .performance(user.id, account_id.into_inner())
        .await
        .map_err(database_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(performance))
}

#[get("/paper-accounts/{account_id}/allocation")]
pub async fn get_paper_account_allocation(
    state: web::Data<AppState>,
    request: HttpRequest,
    account_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let allocation = PaperAccountRepository::new(state.database.clone())
        .allocation(user.id, account_id.into_inner())
        .await
        .map_err(database_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(allocation))
}

#[get("/paper-accounts/{account_id}/performance-history")]
pub async fn get_paper_account_performance_history(
    state: web::Data<AppState>,
    request: HttpRequest,
    account_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let history = PaperAccountRepository::new(state.database.clone())
        .history(user.id, account_id.into_inner(), 365)
        .await
        .map_err(database_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(history))
}

#[get("/paper-accounts/{account_id}/trades")]
pub async fn list_paper_account_trades(
    state: web::Data<AppState>,
    request: HttpRequest,
    account_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let trades = PaperAccountRepository::new(state.database.clone())
        .trades(user.id, account_id.into_inner(), 100)
        .await
        .map_err(database_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(trades))
}

#[get("/paper-accounts/{account_id}/instrument-performance")]
pub async fn get_instrument_performance(
    state: web::Data<AppState>,
    request: HttpRequest,
    account_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let performance = InstrumentPortfolioRepository::new(state.database.clone())
        .performance(user.id, account_id.into_inner())
        .await
        .map_err(database_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(performance))
}

#[get("/paper-accounts/{account_id}/instrument-trades")]
pub async fn list_instrument_trades(
    state: web::Data<AppState>,
    request: HttpRequest,
    account_id: web::Path<Uuid>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let trades = InstrumentPortfolioRepository::new(state.database.clone())
        .trades(user.id, account_id.into_inner(), 100)
        .await
        .map_err(database_error)?
        .ok_or(ApiError::NotFound)?;
    Ok(HttpResponse::Ok().json(trades))
}

#[post("/paper-accounts/{account_id}/instrument-orders")]
pub async fn execute_instrument_order(
    state: web::Data<AppState>,
    request: HttpRequest,
    account_id: web::Path<Uuid>,
    body: web::Json<InstrumentPaperOrderRequest>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let repository = InstrumentPortfolioRepository::new(state.database.clone());
    let account_id = account_id.into_inner();
    let trade = match body.side.trim().to_lowercase().as_str() {
        "buy" => {
            repository
                .buy(
                    user.id,
                    account_id,
                    body.instrument_id,
                    valid_quantity(body.amount, body.units, "amount")?,
                )
                .await
        }
        "sell" => {
            repository
                .sell(
                    user.id,
                    account_id,
                    body.instrument_id,
                    valid_quantity(body.units, body.amount, "units")?,
                )
                .await
        }
        _ => {
            return Err(ApiError::InvalidRequest(
                "side must be buy or sell".to_owned(),
            ));
        }
    }
    .map_err(map_trade_error)?;
    Ok(HttpResponse::Created().json(trade))
}

#[post("/paper-accounts/{account_id}/orders")]
pub async fn execute_paper_order(
    state: web::Data<AppState>,
    request: HttpRequest,
    account_id: web::Path<Uuid>,
    body: web::Json<PaperOrderRequest>,
) -> Result<HttpResponse, ApiError> {
    let user = authenticate(&state, &request).await?;
    let repository = PaperAccountRepository::new(state.database.clone());
    let account_id = account_id.into_inner();
    let trade = match body.side.trim().to_lowercase().as_str() {
        "buy" => {
            let amount = valid_quantity(body.amount, body.units, "amount")?;
            repository
                .buy(user.id, account_id, body.property_id, amount)
                .await
        }
        "sell" => {
            let units = valid_quantity(body.units, body.amount, "units")?;
            repository
                .sell(user.id, account_id, body.property_id, units)
                .await
        }
        _ => {
            return Err(ApiError::InvalidRequest(
                "side must be buy or sell".to_owned(),
            ));
        }
    }
    .map_err(map_trade_error)?;
    Ok(HttpResponse::Created().json(trade))
}

fn valid_quantity(
    expected: Option<Decimal>,
    unexpected: Option<Decimal>,
    field: &str,
) -> Result<Decimal, ApiError> {
    let Some(value) = expected.filter(|_| unexpected.is_none()) else {
        return Err(ApiError::InvalidRequest(format!(
            "provide exactly one positive {field} for this order side"
        )));
    };
    if value <= Decimal::ZERO || value.scale() > 12 {
        return Err(ApiError::InvalidRequest(format!(
            "{field} must be positive and support at most 12 decimal places"
        )));
    }
    Ok(value)
}

fn validate(name: &str, currency: &str, starting_cash: Decimal) -> Result<(), ApiError> {
    if name.is_empty() || name.chars().count() > 100 {
        return Err(ApiError::InvalidRequest(
            "name must contain between 1 and 100 characters".to_owned(),
        ));
    }
    if currency.len() != 3
        || !currency
            .chars()
            .all(|character| character.is_ascii_uppercase())
    {
        return Err(ApiError::InvalidRequest(
            "base_currency must be a three-letter ISO currency code".to_owned(),
        ));
    }
    if starting_cash <= Decimal::ZERO || starting_cash > Decimal::new(1_000_000_000, 0) {
        return Err(ApiError::InvalidRequest(
            "starting_cash must be greater than zero and no more than 1000000000".to_owned(),
        ));
    }
    if starting_cash.scale() > 4 {
        return Err(ApiError::InvalidRequest(
            "starting_cash supports at most four decimal places".to_owned(),
        ));
    }
    Ok(())
}

fn default_account_name() -> String {
    "Demo Portfolio".to_owned()
}

fn default_currency() -> String {
    "USD".to_owned()
}

fn default_demo_cash() -> Decimal {
    DEFAULT_DEMO_CASH
}

fn map_create_error(error: sqlx::Error) -> ApiError {
    if let sqlx::Error::Database(database_error) = &error
        && database_error.is_unique_violation()
    {
        return ApiError::Conflict("a paper account with this name already exists".to_owned());
    }
    database_error(error)
}

fn database_error(error: sqlx::Error) -> ApiError {
    error!(?error, "paper account database operation failed");
    ApiError::Internal
}

fn map_trade_error(error: PaperTradeError) -> ApiError {
    match error {
        PaperTradeError::AccountNotFound | PaperTradeError::PriceUnavailable => ApiError::NotFound,
        PaperTradeError::CurrencyMismatch => ApiError::InvalidRequest(
            "asset currency must match the paper account until FX conversion is supported"
                .to_owned(),
        ),
        PaperTradeError::InsufficientCash => {
            ApiError::InvalidRequest("paper account has insufficient cash".to_owned())
        }
        PaperTradeError::InsufficientUnits => {
            ApiError::InvalidRequest("paper account has insufficient units".to_owned())
        }
        PaperTradeError::Database(error) => database_error(error),
    }
}
