use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

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
        Ok(Some(PaperAccountDetail {
            account,
            cash_ledger,
        }))
    }
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
