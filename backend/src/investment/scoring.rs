use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::analysis::CalculationError;

const ONE_HUNDRED: Decimal = Decimal::from_parts(100, 0, 0, false, 0);
const TWELVE: Decimal = Decimal::from_parts(12, 0, 0, false, 0);
const FIFTEEN: Decimal = Decimal::from_parts(15, 0, 0, false, 0);
const FIVE: Decimal = Decimal::from_parts(5, 0, 0, false, 0);
const ONE_EIGHTY: Decimal = Decimal::from_parts(180, 0, 0, false, 0);

#[derive(Clone, Debug, Deserialize)]
pub struct PropertyScoreInputs {
    pub gross_rental_yield_percent: Option<Decimal>,
    pub annual_growth_percent: Option<Decimal>,
    pub days_on_market: Option<Decimal>,
    pub location: Option<LocationFeatureCounts>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LocationFeatureCounts {
    #[serde(default)]
    pub transport: u16,
    #[serde(default)]
    pub education: u16,
    #[serde(default)]
    pub healthcare: u16,
    #[serde(default)]
    pub commerce: u16,
}

impl PropertyScoreInputs {
    pub fn calculate(&self) -> Result<PropertyScore, CalculationError> {
        validate_optional_range(
            "gross_rental_yield_percent",
            self.gross_rental_yield_percent,
            Decimal::ZERO,
            ONE_HUNDRED,
        )?;
        validate_optional_range(
            "annual_growth_percent",
            self.annual_growth_percent,
            Decimal::new(-100, 0),
            ONE_HUNDRED,
        )?;
        validate_optional_range(
            "days_on_market",
            self.days_on_market,
            Decimal::ZERO,
            Decimal::new(10_000, 0),
        )?;

        let income = self
            .gross_rental_yield_percent
            .map(|value| clamp_ratio(value, TWELVE));
        let growth = self
            .annual_growth_percent
            .map(|value| clamp_ratio(value + FIVE, FIFTEEN + FIVE));
        let liquidity = self.days_on_market.map(|value| {
            clamp(
                ONE_HUNDRED - value * ONE_HUNDRED / ONE_EIGHTY,
                Decimal::ZERO,
                ONE_HUNDRED,
            )
        });
        let location = self.location.as_ref().map(location_score);

        let components = vec![
            ScoreComponent::new(
                "income",
                income,
                "Gross rental yield: 0% maps to 0 and 12% or more maps to 100.",
            ),
            ScoreComponent::new(
                "growth",
                growth,
                "Annual growth: -5% maps to 0 and 15% or more maps to 100.",
            ),
            ScoreComponent::new(
                "liquidity",
                liquidity,
                "Days on market: 0 days maps to 100 and 180 or more maps to 0.",
            ),
            ScoreComponent::new(
                "location",
                location,
                "Each of transport, education, healthcare, and commerce contributes up to 25 points from up to three cached features.",
            ),
            ScoreComponent::new(
                "risk",
                None,
                "Unavailable until verified risk, financing, and environmental inputs are present.",
            ),
        ];
        let weighted = [
            (income, 30_i64),
            (growth, 25),
            (liquidity, 20),
            (location, 25),
        ];
        let available_weight: i64 = weighted
            .iter()
            .filter_map(|(score, weight)| score.map(|_| *weight))
            .sum();
        let overall_score = (available_weight > 0).then(|| {
            let total = weighted
                .iter()
                .filter_map(|(score, weight)| score.map(|value| value * Decimal::from(*weight)))
                .sum::<Decimal>();
            (total / Decimal::from(available_weight)).round_dp(2)
        });
        let unavailable_components = components
            .iter()
            .filter(|component| component.score.is_none())
            .map(|component| component.name.clone())
            .collect();

        Ok(PropertyScore {
            overall_score,
            components,
            unavailable_components,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PropertyScore {
    pub overall_score: Option<Decimal>,
    pub components: Vec<ScoreComponent>,
    pub unavailable_components: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScoreComponent {
    pub name: String,
    pub score: Option<Decimal>,
    pub methodology: String,
}

impl ScoreComponent {
    fn new(name: &str, score: Option<Decimal>, methodology: &str) -> Self {
        Self {
            name: name.to_owned(),
            score: score.map(|value| value.round_dp(2)),
            methodology: methodology.to_owned(),
        }
    }
}

fn location_score(counts: &LocationFeatureCounts) -> Decimal {
    let total = [
        counts.transport,
        counts.education,
        counts.healthcare,
        counts.commerce,
    ]
    .into_iter()
    .map(|count| Decimal::from(count.min(3)) * Decimal::new(25, 0) / Decimal::from(3))
    .sum::<Decimal>();
    clamp(total, Decimal::ZERO, ONE_HUNDRED)
}

fn clamp_ratio(value: Decimal, maximum: Decimal) -> Decimal {
    clamp(value * ONE_HUNDRED / maximum, Decimal::ZERO, ONE_HUNDRED)
}

fn clamp(value: Decimal, minimum: Decimal, maximum: Decimal) -> Decimal {
    value.max(minimum).min(maximum)
}

fn validate_optional_range(
    name: &str,
    value: Option<Decimal>,
    minimum: Decimal,
    maximum: Decimal,
) -> Result<(), CalculationError> {
    if value.is_some_and(|value| value < minimum || value > maximum) {
        return Err(CalculationError::InvalidInput(format!(
            "{name} must be between {minimum} and {maximum}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{LocationFeatureCounts, PropertyScoreInputs};
    use rust_decimal::Decimal;

    #[test]
    fn calculates_only_from_available_explainable_components() {
        let result = PropertyScoreInputs {
            gross_rental_yield_percent: Some(Decimal::new(84, 1)),
            annual_growth_percent: None,
            days_on_market: Some(Decimal::new(30, 0)),
            location: Some(LocationFeatureCounts {
                transport: 3,
                education: 2,
                healthcare: 1,
                commerce: 3,
            }),
        }
        .calculate()
        .expect("score");
        assert!(result.overall_score.is_some());
        assert_eq!(result.unavailable_components, vec!["growth", "risk"]);
    }

    #[test]
    fn rejects_impossible_yield() {
        let result = PropertyScoreInputs {
            gross_rental_yield_percent: Some(Decimal::new(101, 0)),
            annual_growth_percent: None,
            days_on_market: None,
            location: None,
        }
        .calculate();
        assert!(result.is_err());
    }
}
