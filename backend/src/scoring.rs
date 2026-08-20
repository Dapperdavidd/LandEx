use crate::repository::{
    market::MarketRepository, property::PropertyRepository, score::ScoreRepository,
};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Clone)]
pub struct ScoreRefreshService {
    pool: PgPool,
}

#[derive(Debug, Default, Serialize)]
pub struct ScoreRefreshReport {
    pub properties_recorded: usize,
    pub markets_recorded: usize,
    pub unavailable: usize,
}

impl ScoreRefreshService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn refresh(&self) -> Result<ScoreRefreshReport, Box<dyn std::error::Error>> {
        let scores = ScoreRepository::new(self.pool.clone());
        let properties = PropertyRepository::new(self.pool.clone());
        let markets = MarketRepository::new(self.pool.clone());
        let mut report = ScoreRefreshReport::default();
        for id in scores.property_ids().await? {
            if let Some(inputs) = properties.score_inputs(id).await? {
                let score = inputs.calculate()?;
                scores
                    .upsert(
                        Some(id),
                        None,
                        score.overall_score,
                        serde_json::to_value(&score.components)?,
                        serde_json::to_value(&score.unavailable_components)?,
                    )
                    .await?;
                report.properties_recorded += 1;
            } else {
                report.unavailable += 1;
            }
        }
        for id in scores.market_ids().await? {
            if let Some(inputs) = markets.score_inputs(id).await? {
                let score = inputs.calculate()?;
                scores
                    .upsert(
                        None,
                        Some(id),
                        score.overall_score,
                        serde_json::to_value(&score.components)?,
                        serde_json::to_value(&score.unavailable_components)?,
                    )
                    .await?;
                report.markets_recorded += 1;
            } else {
                report.unavailable += 1;
            }
        }
        Ok(report)
    }
}
