use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;

use super::{CurrencyRate, StoredCurrencyRate};

#[derive(Clone)]
pub struct CurrencyRateRepository {
    pool: PgPool,
}

impl CurrencyRateRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, rates: &[CurrencyRate]) -> Result<u64, sqlx::Error> {
        let mut transaction = self.pool.begin().await?;
        let mut affected = 0;
        for rate in rates {
            affected += sqlx::query(
                r#"INSERT INTO currency_rates (provider, base_currency, quote_currency, rate, observed_on)
                   VALUES ($1,$2,$3,$4,$5)
                   ON CONFLICT (provider, base_currency, quote_currency, observed_on)
                   DO UPDATE SET rate = EXCLUDED.rate, created_at = NOW()"#,
            )
            .bind(&rate.provider).bind(&rate.base_currency).bind(&rate.quote_currency)
            .bind(rate.rate).bind(rate.observed_on)
            .execute(&mut *transaction).await?.rows_affected();
        }
        transaction.commit().await?;
        Ok(affected)
    }

    pub async fn latest(
        &self,
        base: &str,
        quote: &str,
        on_or_before: Option<NaiveDate>,
    ) -> Result<Option<StoredCurrencyRate>, sqlx::Error> {
        if let Some(rate) = self.latest_direct(base, quote, on_or_before).await? {
            return Ok(Some(rate));
        }
        let Some(reverse) = self.latest_direct(quote, base, on_or_before).await? else {
            return Ok(None);
        };
        Ok(Some(StoredCurrencyRate {
            provider: format!("{}:inverse", reverse.provider),
            base_currency: base.to_owned(),
            quote_currency: quote.to_owned(),
            rate: Decimal::ONE / reverse.rate,
            observed_on: reverse.observed_on,
        }))
    }

    async fn latest_direct(
        &self,
        base: &str,
        quote: &str,
        on_or_before: Option<NaiveDate>,
    ) -> Result<Option<StoredCurrencyRate>, sqlx::Error> {
        sqlx::query_as(
            r#"SELECT provider, base_currency, quote_currency, rate, observed_on
               FROM currency_rates
               WHERE base_currency = $1 AND quote_currency = $2
                 AND ($3::DATE IS NULL OR observed_on <= $3)
               ORDER BY observed_on DESC, created_at DESC LIMIT 1"#,
        )
        .bind(base)
        .bind(quote)
        .bind(on_or_before)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn convert(
        &self,
        amount: Decimal,
        base: &str,
        quote: &str,
        date: Option<NaiveDate>,
    ) -> Result<Option<(Decimal, StoredCurrencyRate)>, sqlx::Error> {
        if base == quote {
            return Ok(Some((
                amount,
                StoredCurrencyRate {
                    provider: "identity".to_owned(),
                    base_currency: base.to_owned(),
                    quote_currency: quote.to_owned(),
                    rate: Decimal::ONE,
                    observed_on: date.unwrap_or_else(|| chrono::Utc::now().date_naive()),
                },
            )));
        }
        let Some(rate) = self.latest(base, quote, date).await? else {
            return Ok(None);
        };
        Ok(Some(((amount * rate.rate).round_dp(4), rate)))
    }
}
