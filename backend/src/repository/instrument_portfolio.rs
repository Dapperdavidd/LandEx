use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

use super::paper_account::PaperTradeError;

#[derive(Clone)]
pub struct InstrumentPortfolioRepository {
    pool: PgPool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct InstrumentPaperTrade {
    pub id: Uuid,
    pub instrument_id: Uuid,
    pub side: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub units: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub execution_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub gross_amount: Decimal,
    pub settlement_currency: String,
    pub observed_on: NaiveDate,
    pub executed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InstrumentPortfolioPerformance {
    pub account_id: Uuid,
    pub settlement_currency: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub positions_value: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    pub positions: Vec<InstrumentPosition>,
}

#[derive(Debug, Serialize)]
pub struct InstrumentPosition {
    pub instrument_id: Uuid,
    pub name: String,
    pub country_code: String,
    pub instrument_kind: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub units: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub average_entry_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub current_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub cost_basis: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub market_value: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub unrealized_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub return_percent: Decimal,
    pub settlement_currency: String,
    pub observed_on: NaiveDate,
}

#[derive(Debug, sqlx::FromRow)]
struct CalculationTrade {
    instrument_id: Uuid,
    side: String,
    units: Decimal,
    gross_amount: Decimal,
}
#[derive(Debug, sqlx::FromRow)]
struct LatestMark {
    instrument_id: Uuid,
    name: String,
    country_code: String,
    instrument_kind: String,
    current_price: Decimal,
    observed_on: NaiveDate,
}
#[derive(Debug, sqlx::FromRow)]
struct TradeMark {
    value: Decimal,
    observed_on: NaiveDate,
    currency: String,
    instrument_kind: String,
}
#[derive(Default)]
struct PositionState {
    units: Decimal,
    cost_basis: Decimal,
}

impl InstrumentPortfolioRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn trades(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        limit: i64,
    ) -> Result<Option<Vec<InstrumentPaperTrade>>, sqlx::Error> {
        if !account_exists(&self.pool, user_id, account_id).await? {
            return Ok(None);
        }
        Ok(Some(sqlx::query_as("SELECT id, instrument_id, side, units, execution_price, gross_amount, settlement_currency, observed_on, executed_at FROM paper_instrument_trades WHERE account_id=$1 ORDER BY executed_at DESC, id DESC LIMIT $2").bind(account_id).bind(limit).fetch_all(&self.pool).await?))
    }

    pub async fn performance(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> Result<Option<InstrumentPortfolioPerformance>, sqlx::Error> {
        let currency: Option<String> = sqlx::query_scalar(
            "SELECT base_currency FROM paper_accounts WHERE id=$1 AND user_id=$2",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(currency) = currency else {
            return Ok(None);
        };
        let trades: Vec<CalculationTrade> = sqlx::query_as("SELECT instrument_id, side, units, gross_amount FROM paper_instrument_trades WHERE account_id=$1 ORDER BY executed_at, id").bind(account_id).fetch_all(&self.pool).await?;
        let mut states = HashMap::<Uuid, PositionState>::new();
        let mut realized = Decimal::ZERO;
        for trade in trades {
            let state = states.entry(trade.instrument_id).or_default();
            if trade.side == "buy" {
                state.units += trade.units;
                state.cost_basis += trade.gross_amount;
            } else if state.units > Decimal::ZERO {
                let average = state.cost_basis / state.units;
                realized += trade.gross_amount - average * trade.units;
                state.units -= trade.units;
                state.cost_basis -= average * trade.units;
            }
        }
        let ids = states.keys().copied().collect::<Vec<_>>();
        let marks: Vec<LatestMark> = sqlx::query_as("SELECT DISTINCT ON (i.id) i.id AS instrument_id, i.name, i.country_code, i.instrument_kind, o.value AS current_price, o.observed_on FROM investment_instruments i JOIN instrument_observations o ON o.instrument_id=i.id WHERE i.id=ANY($1) ORDER BY i.id, o.observed_on DESC").bind(ids).fetch_all(&self.pool).await?;
        let mut positions = Vec::new();
        for mark in marks {
            let Some(state) = states.get(&mark.instrument_id) else {
                continue;
            };
            if state.units <= Decimal::ZERO {
                continue;
            }
            let average = state.cost_basis / state.units;
            let market_value = state.units * mark.current_price;
            let pnl = market_value - state.cost_basis;
            positions.push(InstrumentPosition {
                instrument_id: mark.instrument_id,
                name: mark.name,
                country_code: mark.country_code,
                instrument_kind: mark.instrument_kind,
                units: state.units,
                average_entry_price: average,
                current_price: mark.current_price,
                cost_basis: state.cost_basis,
                market_value,
                unrealized_pnl: pnl,
                return_percent: percentage(pnl, state.cost_basis),
                settlement_currency: currency.clone(),
                observed_on: mark.observed_on,
            });
        }
        positions.sort_by_key(|position| std::cmp::Reverse(position.market_value));
        let positions_value = positions.iter().map(|position| position.market_value).sum();
        let unrealized_pnl = positions
            .iter()
            .map(|position| position.unrealized_pnl)
            .sum();
        Ok(Some(InstrumentPortfolioPerformance {
            account_id,
            settlement_currency: currency,
            positions_value,
            unrealized_pnl,
            realized_pnl: realized,
            positions,
        }))
    }

    pub async fn buy(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        instrument_id: Uuid,
        amount: Decimal,
    ) -> Result<InstrumentPaperTrade, PaperTradeError> {
        let mut tx = self.pool.begin().await?;
        let currency = lock_account(&mut tx, user_id, account_id).await?;
        let mark = latest_mark(&mut tx, instrument_id).await?;
        if mark.instrument_kind == "listed_security" && mark.currency != currency {
            return Err(PaperTradeError::CurrencyMismatch);
        }
        let price = mark.value;
        let observed_on = mark.observed_on;
        let cash: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount),0) FROM paper_cash_ledger WHERE account_id=$1",
        )
        .bind(account_id)
        .fetch_one(&mut *tx)
        .await?;
        if cash < amount {
            return Err(PaperTradeError::InsufficientCash);
        }
        let units = amount / price;
        let trade = insert_trade(
            &mut tx,
            account_id,
            instrument_id,
            "buy",
            units,
            price,
            amount,
            &currency,
            observed_on,
        )
        .await?;
        cash_entry(&mut tx, account_id, trade.id, "purchase", -amount).await?;
        tx.commit().await?;
        Ok(trade)
    }

    pub async fn sell(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        instrument_id: Uuid,
        units: Decimal,
    ) -> Result<InstrumentPaperTrade, PaperTradeError> {
        let mut tx = self.pool.begin().await?;
        let currency = lock_account(&mut tx, user_id, account_id).await?;
        let held: Decimal = sqlx::query_scalar("SELECT COALESCE(SUM(CASE WHEN side='buy' THEN units ELSE -units END),0) FROM paper_instrument_trades WHERE account_id=$1 AND instrument_id=$2").bind(account_id).bind(instrument_id).fetch_one(&mut *tx).await?;
        if held < units {
            return Err(PaperTradeError::InsufficientUnits);
        }
        let mark = latest_mark(&mut tx, instrument_id).await?;
        if mark.instrument_kind == "listed_security" && mark.currency != currency {
            return Err(PaperTradeError::CurrencyMismatch);
        }
        let price = mark.value;
        let observed_on = mark.observed_on;
        let amount = (units * price).round_dp(4);
        let trade = insert_trade(
            &mut tx,
            account_id,
            instrument_id,
            "sell",
            units,
            price,
            amount,
            &currency,
            observed_on,
        )
        .await?;
        cash_entry(&mut tx, account_id, trade.id, "sale", amount).await?;
        tx.commit().await?;
        Ok(trade)
    }
}

async fn account_exists(
    pool: &PgPool,
    user_id: Uuid,
    account_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM paper_accounts WHERE id=$1 AND user_id=$2)")
        .bind(account_id)
        .bind(user_id)
        .fetch_one(pool)
        .await
}
async fn lock_account(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    account_id: Uuid,
) -> Result<String, PaperTradeError> {
    sqlx::query_scalar("SELECT base_currency FROM paper_accounts WHERE id=$1 AND user_id=$2 AND status='active' FOR UPDATE").bind(account_id).bind(user_id).fetch_optional(&mut **tx).await?.ok_or(PaperTradeError::AccountNotFound)
}
async fn latest_mark(
    tx: &mut Transaction<'_, Postgres>,
    instrument_id: Uuid,
) -> Result<TradeMark, PaperTradeError> {
    sqlx::query_as("SELECT o.value, o.observed_on, o.currency, i.instrument_kind FROM investment_instruments i JOIN instrument_observations o ON o.instrument_id=i.id WHERE i.id=$1 AND i.status='paper_tradeable' AND o.value>0 ORDER BY o.observed_on DESC LIMIT 1").bind(instrument_id).fetch_optional(&mut **tx).await?.ok_or(PaperTradeError::PriceUnavailable)
}
#[allow(clippy::too_many_arguments)]
async fn insert_trade(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    instrument_id: Uuid,
    side: &str,
    units: Decimal,
    price: Decimal,
    amount: Decimal,
    currency: &str,
    observed_on: NaiveDate,
) -> Result<InstrumentPaperTrade, sqlx::Error> {
    sqlx::query_as("INSERT INTO paper_instrument_trades(account_id,instrument_id,side,units,execution_price,gross_amount,settlement_currency,observed_on) VALUES($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id,instrument_id,side,units,execution_price,gross_amount,settlement_currency,observed_on,executed_at").bind(account_id).bind(instrument_id).bind(side).bind(units).bind(price).bind(amount).bind(currency).bind(observed_on).fetch_one(&mut **tx).await
}
async fn cash_entry(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    trade_id: Uuid,
    entry_type: &str,
    amount: Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO paper_cash_ledger(account_id,entry_type,amount,description,reference_id) VALUES($1,$2,$3,'Paper market instrument trade',$4)").bind(account_id).bind(entry_type).bind(amount).bind(trade_id).execute(&mut **tx).await?;
    Ok(())
}
fn percentage(numerator: Decimal, denominator: Decimal) -> Decimal {
    if denominator == Decimal::ZERO {
        Decimal::ZERO
    } else {
        (numerator / denominator * Decimal::new(100, 0)).round_dp(4)
    }
}
