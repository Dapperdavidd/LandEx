use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum PaperTradeError {
    #[error("paper account was not found")]
    AccountNotFound,
    #[error("property has no current sale valuation")]
    PriceUnavailable,
    #[error("property currency does not match the paper account")]
    CurrencyMismatch,
    #[error("paper account does not have enough cash")]
    InsufficientCash,
    #[error("paper account does not hold enough units")]
    InsufficientUnits,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Clone)]
pub struct PaperAccountRepository {
    pool: PgPool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PaperAccount {
    pub id: Uuid,
    pub name: String,
    pub base_currency: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub cash_balance: Decimal,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CashLedgerEntry {
    pub id: Uuid,
    pub entry_type: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub amount: Decimal,
    pub description: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PaperAccountDetail {
    #[serde(flatten)]
    pub account: PaperAccount,
    pub cash_ledger: Vec<CashLedgerEntry>,
    pub positions: Vec<PaperPosition>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PaperPosition {
    pub property_id: Uuid,
    #[serde(with = "rust_decimal::serde::str")]
    pub units: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub current_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub market_value: Decimal,
    pub currency: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PaperTrade {
    pub id: Uuid,
    pub property_id: Uuid,
    pub side: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub units: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub execution_price: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub gross_amount: Decimal,
    pub currency: String,
    pub executed_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct PortfolioPerformance {
    pub account_id: Uuid,
    pub base_currency: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub cash_balance: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub positions_value: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub total_value: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub net_funding: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub total_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub total_return_percent: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
    pub positions: Vec<PositionPerformance>,
}

#[derive(Debug, Serialize)]
pub struct PositionPerformance {
    pub property_id: Uuid,
    pub property_type: String,
    pub country_code: String,
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
    pub currency: String,
}

#[derive(Debug, Serialize)]
pub struct PortfolioAllocation {
    pub account_id: Uuid,
    #[serde(with = "rust_decimal::serde::str")]
    pub positions_value: Decimal,
    pub by_country: Vec<AllocationSlice>,
    pub by_property_type: Vec<AllocationSlice>,
    pub unavailable_dimensions: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct AllocationSlice {
    pub label: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub market_value: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub percentage: Decimal,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct PortfolioSnapshot {
    pub observed_on: NaiveDate,
    pub base_currency: String,
    #[serde(with = "rust_decimal::serde::str")]
    pub cash_balance: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub positions_value: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub total_value: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub net_funding: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub total_pnl: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub total_return_percent: Decimal,
    #[serde(with = "rust_decimal::serde::str")]
    pub realized_pnl: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct TradeCalculationRow {
    property_id: Uuid,
    side: String,
    units: Decimal,
    gross_amount: Decimal,
}

#[derive(Debug, sqlx::FromRow)]
struct LatestPriceRow {
    property_id: Uuid,
    property_type: String,
    country_code: String,
    current_price: Decimal,
    currency: String,
}

#[derive(Default)]
struct PositionState {
    units: Decimal,
    cost_basis: Decimal,
}

impl PaperAccountRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        user_id: Uuid,
        name: &str,
        base_currency: &str,
        starting_cash: Decimal,
    ) -> Result<PaperAccount, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let account_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO paper_accounts (user_id, name, base_currency)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(name)
        .bind(base_currency)
        .fetch_one(&mut *transaction)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO paper_cash_ledger (account_id, entry_type, amount, description)
            VALUES ($1, 'initial_funding', $2, 'Initial demo capital')
            "#,
        )
        .bind(account_id)
        .bind(starting_cash)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        self.find(user_id, account_id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn list(&self, user_id: Uuid) -> Result<Vec<PaperAccount>, sqlx::Error> {
        sqlx::query_as(&account_query(
            "WHERE paper_accounts.user_id = $1 GROUP BY paper_accounts.id ORDER BY paper_accounts.created_at DESC",
        ))
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn find(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> Result<Option<PaperAccount>, sqlx::Error> {
        sqlx::query_as(&account_query(
            "WHERE paper_accounts.user_id = $1 AND paper_accounts.id = $2 GROUP BY paper_accounts.id",
        ))
        .bind(user_id)
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn detail(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> Result<Option<PaperAccountDetail>, sqlx::Error> {
        let Some(account) = self.find(user_id, account_id).await? else {
            return Ok(None);
        };
        let cash_ledger = sqlx::query_as(
            r#"
            SELECT id, entry_type, amount, description, created_at
            FROM paper_cash_ledger
            WHERE account_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT 100
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;
        let positions = self.positions(user_id, account_id).await?;
        Ok(Some(PaperAccountDetail {
            account,
            cash_ledger,
            positions,
        }))
    }

    pub async fn positions(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> Result<Vec<PaperPosition>, sqlx::Error> {
        sqlx::query_as(
            r#"
            WITH holdings AS (
                SELECT paper_trades.property_id,
                       SUM(CASE WHEN side = 'buy' THEN units ELSE -units END) AS units
                FROM paper_trades
                JOIN paper_accounts ON paper_accounts.id = paper_trades.account_id
                WHERE paper_trades.account_id = $1 AND paper_accounts.user_id = $2
                GROUP BY paper_trades.property_id
            ), latest_price AS (
                SELECT DISTINCT ON (property_id) property_id,
                       COALESCE(asking_price, estimated_value) AS current_price, currency
                FROM property_observations
                WHERE COALESCE(asking_price, estimated_value) IS NOT NULL
                ORDER BY property_id, observed_on DESC, created_at DESC
            )
            SELECT holdings.property_id, holdings.units, latest_price.current_price,
                   (holdings.units * latest_price.current_price) AS market_value,
                   latest_price.currency
            FROM holdings JOIN latest_price USING (property_id)
            WHERE holdings.units > 0
            ORDER BY market_value DESC
            "#,
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn trades(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        limit: i64,
    ) -> Result<Option<Vec<PaperTrade>>, sqlx::Error> {
        if self.find(user_id, account_id).await?.is_none() {
            return Ok(None);
        }
        let trades = sqlx::query_as(
            r#"
            SELECT paper_trades.id, paper_trades.property_id, paper_trades.side,
                   paper_trades.units, paper_trades.execution_price,
                   paper_trades.gross_amount, paper_trades.currency, paper_trades.executed_at
            FROM paper_trades
            WHERE account_id = $1
            ORDER BY executed_at DESC, id DESC
            LIMIT $2
            "#,
        )
        .bind(account_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(Some(trades))
    }

    pub async fn performance(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> Result<Option<PortfolioPerformance>, sqlx::Error> {
        let Some(account) = self.find(user_id, account_id).await? else {
            return Ok(None);
        };
        let trades: Vec<TradeCalculationRow> = sqlx::query_as(
            r#"
            SELECT property_id, side, units, gross_amount
            FROM paper_trades
            WHERE account_id = $1
            ORDER BY executed_at, id
            "#,
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;

        let mut states: HashMap<Uuid, PositionState> = HashMap::new();
        let mut realized_pnl = Decimal::ZERO;
        for trade in trades {
            let state = states.entry(trade.property_id).or_default();
            if trade.side == "buy" {
                state.units += trade.units;
                state.cost_basis += trade.gross_amount;
            } else if state.units > Decimal::ZERO {
                let average_cost = state.cost_basis / state.units;
                realized_pnl += trade.gross_amount - (average_cost * trade.units);
                state.units -= trade.units;
                state.cost_basis -= average_cost * trade.units;
            }
        }

        let prices: Vec<LatestPriceRow> = sqlx::query_as(
            r#"
            SELECT DISTINCT ON (properties.id) properties.id AS property_id,
                   properties.property_type, locations.country_code,
                   COALESCE(property_observations.asking_price, property_observations.estimated_value) AS current_price,
                   property_observations.currency
            FROM properties
            JOIN locations ON locations.id = properties.location_id
            JOIN property_observations ON property_observations.property_id = properties.id
            WHERE properties.id = ANY($1)
              AND COALESCE(property_observations.asking_price, property_observations.estimated_value) IS NOT NULL
            ORDER BY properties.id, property_observations.observed_on DESC,
                     property_observations.created_at DESC
            "#,
        )
        .bind(states.keys().copied().collect::<Vec<_>>())
        .fetch_all(&self.pool)
        .await?;

        let mut positions = Vec::new();
        for price in prices {
            let Some(state) = states.get(&price.property_id) else {
                continue;
            };
            if state.units <= Decimal::ZERO {
                continue;
            }
            let average_entry_price = state.cost_basis / state.units;
            let market_value = state.units * price.current_price;
            let unrealized_pnl = market_value - state.cost_basis;
            let return_percent = percentage(unrealized_pnl, state.cost_basis);
            positions.push(PositionPerformance {
                property_id: price.property_id,
                property_type: price.property_type,
                country_code: price.country_code,
                units: state.units,
                average_entry_price,
                current_price: price.current_price,
                cost_basis: state.cost_basis,
                market_value,
                unrealized_pnl,
                return_percent,
                currency: price.currency,
            });
        }
        positions.sort_by_key(|position| std::cmp::Reverse(position.market_value));

        let positions_value = positions.iter().map(|position| position.market_value).sum();
        let net_funding: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount), 0) FROM paper_cash_ledger WHERE account_id = $1 AND entry_type IN ('initial_funding', 'adjustment')",
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?;
        let total_value = account.cash_balance + positions_value;
        let total_pnl = total_value - net_funding;

        Ok(Some(PortfolioPerformance {
            account_id,
            base_currency: account.base_currency,
            cash_balance: account.cash_balance,
            positions_value,
            total_value,
            net_funding,
            total_pnl,
            total_return_percent: percentage(total_pnl, net_funding),
            realized_pnl,
            positions,
        }))
    }

    pub async fn allocation(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> Result<Option<PortfolioAllocation>, sqlx::Error> {
        let Some(performance) = self.performance(user_id, account_id).await? else {
            return Ok(None);
        };
        let mut countries: HashMap<String, Decimal> = HashMap::new();
        let mut property_types: HashMap<String, Decimal> = HashMap::new();
        for position in &performance.positions {
            *countries.entry(position.country_code.clone()).or_default() += position.market_value;
            *property_types
                .entry(position.property_type.clone())
                .or_default() += position.market_value;
        }
        Ok(Some(PortfolioAllocation {
            account_id,
            positions_value: performance.positions_value,
            by_country: allocation_slices(countries, performance.positions_value),
            by_property_type: allocation_slices(property_types, performance.positions_value),
            unavailable_dimensions: vec!["strategy".to_owned(), "risk".to_owned()],
        }))
    }

    pub async fn history(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        limit: i64,
    ) -> Result<Option<Vec<PortfolioSnapshot>>, sqlx::Error> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM paper_accounts WHERE id = $1 AND user_id = $2)",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            return Ok(None);
        }
        let snapshots = sqlx::query_as(
            "SELECT observed_on, base_currency, cash_balance, positions_value, total_value, net_funding, total_pnl, total_return_percent, realized_pnl FROM paper_account_snapshots WHERE account_id = $1 ORDER BY observed_on DESC LIMIT $2",
        )
        .bind(account_id)
        .bind(limit.clamp(1, 3650))
        .fetch_all(&self.pool)
        .await?;
        Ok(Some(snapshots))
    }

    pub async fn record_snapshot(
        &self,
        user_id: Uuid,
        account_id: Uuid,
    ) -> Result<Option<PortfolioSnapshot>, sqlx::Error> {
        let Some(performance) = self.performance(user_id, account_id).await? else {
            return Ok(None);
        };
        let snapshot = sqlx::query_as(
            "INSERT INTO paper_account_snapshots (account_id, observed_on, base_currency, cash_balance, positions_value, total_value, net_funding, total_pnl, total_return_percent, realized_pnl) VALUES ($1, CURRENT_DATE, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (account_id, observed_on) DO UPDATE SET base_currency = EXCLUDED.base_currency, cash_balance = EXCLUDED.cash_balance, positions_value = EXCLUDED.positions_value, total_value = EXCLUDED.total_value, net_funding = EXCLUDED.net_funding, total_pnl = EXCLUDED.total_pnl, total_return_percent = EXCLUDED.total_return_percent, realized_pnl = EXCLUDED.realized_pnl, updated_at = NOW() RETURNING observed_on, base_currency, cash_balance, positions_value, total_value, net_funding, total_pnl, total_return_percent, realized_pnl",
        )
        .bind(account_id)
        .bind(&performance.base_currency)
        .bind(performance.cash_balance)
        .bind(performance.positions_value)
        .bind(performance.total_value)
        .bind(performance.net_funding)
        .bind(performance.total_pnl)
        .bind(performance.total_return_percent)
        .bind(performance.realized_pnl)
        .fetch_one(&self.pool)
        .await?;
        Ok(Some(snapshot))
    }

    pub async fn active_account_owners(&self) -> Result<Vec<(Uuid, Uuid)>, sqlx::Error> {
        sqlx::query_as("SELECT user_id, id FROM paper_accounts WHERE status = 'active'")
            .fetch_all(&self.pool)
            .await
    }

    pub async fn buy(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        property_id: Uuid,
        amount: Decimal,
    ) -> Result<PaperTrade, PaperTradeError> {
        let mut transaction = self.pool.begin().await?;
        let account_currency = lock_account(&mut transaction, user_id, account_id).await?;
        let (price, currency) = latest_property_price(&mut transaction, property_id).await?;
        if currency != account_currency {
            return Err(PaperTradeError::CurrencyMismatch);
        }
        let cash: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount), 0) FROM paper_cash_ledger WHERE account_id = $1",
        )
        .bind(account_id)
        .fetch_one(&mut *transaction)
        .await?;
        if cash < amount {
            return Err(PaperTradeError::InsufficientCash);
        }
        let units = amount / price;
        let trade = insert_trade(
            &mut transaction,
            account_id,
            property_id,
            "buy",
            units,
            price,
            amount,
            &currency,
        )
        .await?;
        insert_cash_entry(&mut transaction, account_id, trade.id, "purchase", -amount).await?;
        transaction.commit().await?;
        Ok(trade)
    }

    pub async fn sell(
        &self,
        user_id: Uuid,
        account_id: Uuid,
        property_id: Uuid,
        units: Decimal,
    ) -> Result<PaperTrade, PaperTradeError> {
        let mut transaction = self.pool.begin().await?;
        let account_currency = lock_account(&mut transaction, user_id, account_id).await?;
        let held_units: Decimal = sqlx::query_scalar(
            "SELECT COALESCE(SUM(CASE WHEN side = 'buy' THEN units ELSE -units END), 0) FROM paper_trades WHERE account_id = $1 AND property_id = $2",
        )
        .bind(account_id)
        .bind(property_id)
        .fetch_one(&mut *transaction)
        .await?;
        if held_units < units {
            return Err(PaperTradeError::InsufficientUnits);
        }
        let (price, currency) = latest_property_price(&mut transaction, property_id).await?;
        if currency != account_currency {
            return Err(PaperTradeError::CurrencyMismatch);
        }
        let amount = (units * price).round_dp(4);
        let trade = insert_trade(
            &mut transaction,
            account_id,
            property_id,
            "sell",
            units,
            price,
            amount,
            &currency,
        )
        .await?;
        insert_cash_entry(&mut transaction, account_id, trade.id, "sale", amount).await?;
        transaction.commit().await?;
        Ok(trade)
    }
}

fn allocation_slices(values: HashMap<String, Decimal>, total: Decimal) -> Vec<AllocationSlice> {
    let mut slices = values
        .into_iter()
        .map(|(label, market_value)| AllocationSlice {
            label,
            market_value,
            percentage: percentage(market_value, total),
        })
        .collect::<Vec<_>>();
    slices.sort_by_key(|slice| std::cmp::Reverse(slice.market_value));
    slices
}

fn percentage(numerator: Decimal, denominator: Decimal) -> Decimal {
    if denominator == Decimal::ZERO {
        Decimal::ZERO
    } else {
        (numerator / denominator * Decimal::new(100, 0)).round_dp(4)
    }
}

async fn lock_account(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    account_id: Uuid,
) -> Result<String, PaperTradeError> {
    sqlx::query_scalar(
        "SELECT base_currency FROM paper_accounts WHERE id = $1 AND user_id = $2 AND status = 'active' FOR UPDATE",
    )
    .bind(account_id)
    .bind(user_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(PaperTradeError::AccountNotFound)
}

async fn latest_property_price(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    property_id: Uuid,
) -> Result<(Decimal, String), PaperTradeError> {
    sqlx::query_as(
        "SELECT COALESCE(asking_price, estimated_value), currency FROM property_observations WHERE property_id = $1 AND COALESCE(asking_price, estimated_value) > 0 ORDER BY observed_on DESC, created_at DESC LIMIT 1",
    )
    .bind(property_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(PaperTradeError::PriceUnavailable)
}

#[allow(clippy::too_many_arguments)]
async fn insert_trade(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: Uuid,
    property_id: Uuid,
    side: &str,
    units: Decimal,
    price: Decimal,
    amount: Decimal,
    currency: &str,
) -> Result<PaperTrade, sqlx::Error> {
    sqlx::query_as(
        "INSERT INTO paper_trades (account_id, property_id, side, units, execution_price, gross_amount, currency) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id, property_id, side, units, execution_price, gross_amount, currency, executed_at",
    )
    .bind(account_id).bind(property_id).bind(side).bind(units).bind(price).bind(amount).bind(currency)
    .fetch_one(&mut **transaction).await
}

async fn insert_cash_entry(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: Uuid,
    trade_id: Uuid,
    entry_type: &str,
    amount: Decimal,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO paper_cash_ledger (account_id, entry_type, amount, description, reference_id) VALUES ($1,$2,$3,'Paper property trade',$4)")
        .bind(account_id).bind(entry_type).bind(amount).bind(trade_id)
        .execute(&mut **transaction).await?;
    Ok(())
}

fn account_query(suffix: &str) -> String {
    format!(
        r#"
        SELECT paper_accounts.id, paper_accounts.name, paper_accounts.base_currency,
               COALESCE(SUM(paper_cash_ledger.amount), 0) AS cash_balance,
               paper_accounts.status, paper_accounts.created_at, paper_accounts.updated_at
        FROM paper_accounts
        LEFT JOIN paper_cash_ledger ON paper_cash_ledger.account_id = paper_accounts.id
        {suffix}
        "#
    )
}
