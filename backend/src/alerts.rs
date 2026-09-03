use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::json;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::{repository::property::PropertyRepository, routes::properties::PropertySearchQuery};

#[derive(Clone)]
pub struct AlertEvaluationService {
    pool: PgPool,
}

#[derive(Debug, Default, Serialize)]
pub struct AlertEvaluationReport {
    pub evaluated: usize,
    pub initialized: usize,
    pub emitted: usize,
    pub unavailable: usize,
}

#[derive(Debug, FromRow)]
struct Rule {
    id: Uuid,
    user_id: Uuid,
    alert_type: String,
    threshold: Option<Decimal>,
    last_numeric_value: Option<Decimal>,
    last_text_value: Option<String>,
    property_id: Option<Uuid>,
    market_id: Option<Uuid>,
    instrument_id: Option<Uuid>,
    saved_search_criteria: Option<serde_json::Value>,
}

enum Snapshot {
    Numeric(Decimal),
    Text(String),
    Matches { signature: String, count: i64 },
}

impl AlertEvaluationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn evaluate(&self) -> Result<AlertEvaluationReport, sqlx::Error> {
        let rules: Vec<Rule> = sqlx::query_as(
            r#"
            SELECT ar.id, ar.user_id, ar.alert_type, ar.threshold,
                   ar.last_numeric_value, ar.last_text_value,
                   wi.property_id, wi.market_id, wi.instrument_id,
                   ss.criteria AS saved_search_criteria
            FROM alert_rules ar
            LEFT JOIN watchlist_items wi ON wi.id=ar.watchlist_item_id
            LEFT JOIN saved_searches ss ON ss.id=ar.saved_search_id
            WHERE ar.enabled
            ORDER BY ar.id
        "#,
        )
        .fetch_all(&self.pool)
        .await?;
        let mut report = AlertEvaluationReport::default();
        for rule in rules {
            report.evaluated += 1;
            let Some(snapshot) = self.snapshot(&rule).await? else {
                report.unavailable += 1;
                continue;
            };
            let initialized = match &snapshot {
                Snapshot::Numeric(_) => rule.last_numeric_value.is_none(),
                Snapshot::Text(_) => rule.last_text_value.is_none(),
                Snapshot::Matches { .. } => rule.last_text_value.is_none(),
            };
            if initialized {
                report.initialized += 1;
            }
            let trigger = match &snapshot {
                Snapshot::Numeric(value) => rule
                    .last_numeric_value
                    .zip(rule.threshold)
                    .is_some_and(|(previous, threshold)| {
                        previous != Decimal::ZERO
                            && ((*value - previous).abs() / previous.abs() * Decimal::from(100))
                                >= threshold
                    }),
                Snapshot::Text(value) => rule
                    .last_text_value
                    .as_ref()
                    .is_some_and(|previous| previous != value),
                Snapshot::Matches { signature, .. } => rule
                    .last_text_value
                    .as_ref()
                    .is_some_and(|previous| previous != signature),
            };
            if trigger && self.emit(&rule, &snapshot).await? {
                report.emitted += 1;
            }
            self.store_baseline(rule.id, &snapshot).await?;
        }
        Ok(report)
    }

    async fn snapshot(&self, rule: &Rule) -> Result<Option<Snapshot>, sqlx::Error> {
        if rule.alert_type == "new_match" {
            let Some(criteria) = rule.saved_search_criteria.clone() else {
                return Ok(None);
            };
            let Ok(mut query) = serde_json::from_value::<PropertySearchQuery>(criteria) else {
                return Ok(None);
            };
            query.limit = Some(100);
            query.offset = Some(0);
            let Ok(filters) = query.into_filters() else {
                return Ok(None);
            };
            let page = PropertyRepository::new(self.pool.clone())
                .search(&filters)
                .await?;
            let signature = page
                .items
                .iter()
                .map(|item| item.id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            return Ok(Some(Snapshot::Matches {
                signature,
                count: page.total,
            }));
        }
        if let Some(property_id) = rule.property_id {
            let value = match rule.alert_type.as_str() {
                "price_change" => numeric(&self.pool, "SELECT price FROM listings WHERE property_id=$1 AND status='active' ORDER BY last_seen_at DESC LIMIT 1", property_id).await?,
                "rent_change" => numeric(&self.pool, "SELECT rental_price_monthly FROM property_observations WHERE property_id=$1 AND rental_price_monthly IS NOT NULL ORDER BY observed_on DESC, created_at DESC LIMIT 1", property_id).await?,
                "yield_change" => numeric(&self.pool, "SELECT (rental_price_monthly*12*100/NULLIF(COALESCE(asking_price,estimated_value),0)) FROM property_observations WHERE property_id=$1 AND rental_price_monthly IS NOT NULL AND COALESCE(asking_price,estimated_value) IS NOT NULL ORDER BY observed_on DESC, created_at DESC LIMIT 1", property_id).await?,
                "listing_status_change" => return Ok(sqlx::query_scalar("SELECT status FROM listings WHERE property_id=$1 ORDER BY last_seen_at DESC LIMIT 1").bind(property_id).fetch_optional(&self.pool).await?.map(Snapshot::Text)),
                _ => None,
            };
            return Ok(value.map(Snapshot::Numeric));
        }
        if rule.alert_type == "market_change"
            && let Some(market_id) = rule.market_id
        {
            return Ok(numeric(&self.pool, "SELECT annual_growth_percent FROM market_observations WHERE market_id=$1 AND annual_growth_percent IS NOT NULL ORDER BY observed_on DESC, created_at DESC LIMIT 1", market_id).await?.map(Snapshot::Numeric));
        }
        if let Some(instrument_id) = rule.instrument_id {
            let value = match rule.alert_type.as_str() {
                "price_change" => numeric(&self.pool, "SELECT value FROM instrument_observations WHERE instrument_id=$1 ORDER BY observed_on DESC, created_at DESC LIMIT 1", instrument_id).await?,
                "market_change" => numeric(&self.pool, "SELECT annual_change_percent FROM instrument_observations WHERE instrument_id=$1 AND annual_change_percent IS NOT NULL ORDER BY observed_on DESC, created_at DESC LIMIT 1", instrument_id).await?,
                _ => None,
            };
            return Ok(value.map(Snapshot::Numeric));
        }
        Ok(None)
    }

    async fn emit(&self, rule: &Rule, snapshot: &Snapshot) -> Result<bool, sqlx::Error> {
        let signature = match snapshot {
            Snapshot::Numeric(value) => value.to_string(),
            Snapshot::Text(value) => value.clone(),
            Snapshot::Matches { signature, .. } => signature.clone(),
        };
        let key = format!("{}:{}:{}", rule.id, rule.alert_type, signature);
        let body = match snapshot {
            Snapshot::Matches { count, .. } => {
                format!("{count} properties now match your saved search")
            }
            _ => format!(
                "{} changed to {}",
                rule.alert_type.replace('_', " "),
                signature
            ),
        };
        Ok(sqlx::query(r#"INSERT INTO notifications (user_id, alert_rule_id, notification_type, title, body, payload, deduplication_key)
            VALUES ($1,$2,'alert','LandEX watchlist alert',$3,$4,$5) ON CONFLICT DO NOTHING"#)
            .bind(rule.user_id).bind(rule.id).bind(body).bind(json!({"alert_type":rule.alert_type,"value":signature})).bind(key)
            .execute(&self.pool).await?.rows_affected()==1)
    }

    async fn store_baseline(&self, id: Uuid, snapshot: &Snapshot) -> Result<(), sqlx::Error> {
        let (numeric, text) = match snapshot {
            Snapshot::Numeric(value) => (Some(*value), None),
            Snapshot::Text(value) => (None, Some(value)),
            Snapshot::Matches { signature, .. } => (None, Some(signature)),
        };
        sqlx::query("UPDATE alert_rules SET last_numeric_value=$2,last_text_value=$3,last_evaluated_at=NOW(),updated_at=NOW() WHERE id=$1")
            .bind(id).bind(numeric).bind(text).execute(&self.pool).await?;
        Ok(())
    }
}

async fn numeric(pool: &PgPool, query: &str, id: Uuid) -> Result<Option<Decimal>, sqlx::Error> {
    sqlx::query_scalar(query)
        .bind(id)
        .fetch_optional(pool)
        .await
}
