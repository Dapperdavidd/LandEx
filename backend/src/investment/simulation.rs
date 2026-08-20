use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::analysis::{CalculationError, InvestmentInputs};

#[derive(Clone, Debug, Deserialize)]
pub struct ScenarioSimulationRequest {
    pub investment: InvestmentInputs,
    #[serde(default = "default_conservative")]
    pub conservative_appreciation_percent: Decimal,
    #[serde(default = "default_base")]
    pub base_appreciation_percent: Decimal,
    #[serde(default = "default_bull")]
    pub bull_appreciation_percent: Decimal,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioSimulation {
    pub holding_years: u16,
    pub scenarios: Vec<ScenarioProjection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioProjection {
    pub name: &'static str,
    pub annual_appreciation_percent: Decimal,
    pub projected_property_value: Decimal,
    pub projected_cumulative_cash_flow: Decimal,
    pub projected_net_sale_proceeds: Decimal,
    pub projected_total_profit: Decimal,
    pub projected_total_return_percent: Decimal,
    pub timeline: Vec<YearProjection>,
}

#[derive(Clone, Debug, Serialize)]
pub struct YearProjection {
    pub year: u16,
    pub projected_property_value: Decimal,
    pub cumulative_cash_flow: Decimal,
    pub total_profit_if_sold: Decimal,
    pub total_return_percent_if_sold: Decimal,
}

impl ScenarioSimulationRequest {
    pub fn calculate(&self) -> Result<ScenarioSimulation, CalculationError> {
        validate_order(
            self.conservative_appreciation_percent,
            self.base_appreciation_percent,
            self.bull_appreciation_percent,
        )?;
        let scenarios = [
            ("conservative", self.conservative_appreciation_percent),
            ("base", self.base_appreciation_percent),
            ("bull", self.bull_appreciation_percent),
        ]
        .into_iter()
        .map(|(name, appreciation)| self.project(name, appreciation))
        .collect::<Result<Vec<_>, _>>()?;
        Ok(ScenarioSimulation {
            holding_years: self.investment.holding_years,
            scenarios,
        })
    }

    fn project(
        &self,
        name: &'static str,
        appreciation: Decimal,
    ) -> Result<ScenarioProjection, CalculationError> {
        let mut timeline = Vec::with_capacity(self.investment.holding_years as usize);
        let mut final_analysis = None;
        for year in 1..=self.investment.holding_years {
            let mut inputs = self.investment.clone();
            inputs.annual_appreciation_percent = appreciation;
            inputs.holding_years = year;
            let analysis = inputs.calculate()?;
            timeline.push(YearProjection {
                year,
                projected_property_value: analysis.projected_property_value,
                cumulative_cash_flow: analysis.projected_cumulative_cash_flow,
                total_profit_if_sold: analysis.projected_total_profit,
                total_return_percent_if_sold: analysis.projected_total_return_percent,
            });
            final_analysis = Some(analysis);
        }
        let analysis = final_analysis.ok_or_else(|| {
            CalculationError::InvalidInput("holding_years must be between 1 and 100".to_owned())
        })?;
        Ok(ScenarioProjection {
            name,
            annual_appreciation_percent: appreciation,
            projected_property_value: analysis.projected_property_value,
            projected_cumulative_cash_flow: analysis.projected_cumulative_cash_flow,
            projected_net_sale_proceeds: analysis.projected_net_sale_proceeds,
            projected_total_profit: analysis.projected_total_profit,
            projected_total_return_percent: analysis.projected_total_return_percent,
            timeline,
        })
    }
}

fn validate_order(
    conservative: Decimal,
    base: Decimal,
    bull: Decimal,
) -> Result<(), CalculationError> {
    if conservative >= base || base >= bull {
        return Err(CalculationError::InvalidInput(
            "scenario appreciation must increase from conservative to base to bull".to_owned(),
        ));
    }
    if conservative <= Decimal::new(-100, 0) {
        return Err(CalculationError::InvalidInput(
            "scenario appreciation must be greater than -100".to_owned(),
        ));
    }
    Ok(())
}

fn default_conservative() -> Decimal {
    Decimal::new(4, 0)
}
fn default_base() -> Decimal {
    Decimal::new(8, 0)
}
fn default_bull() -> Decimal {
    Decimal::new(13, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ScenarioSimulationRequest {
        ScenarioSimulationRequest {
            investment: InvestmentInputs {
                purchase_price: Decimal::new(500_000, 0),
                monthly_rent: Decimal::new(3_500, 0),
                annual_other_income: Decimal::ZERO,
                vacancy_rate_percent: Decimal::new(5, 0),
                annual_property_tax: Decimal::new(6_000, 0),
                annual_insurance: Decimal::new(2_000, 0),
                annual_maintenance: Decimal::new(4_000, 0),
                annual_management: Decimal::new(3_000, 0),
                annual_other_expenses: Decimal::ZERO,
                annual_debt_service: Decimal::ZERO,
                down_payment: None,
                closing_costs: Decimal::new(10_000, 0),
                annual_appreciation_percent: Decimal::ZERO,
                holding_years: 5,
                sale_cost_percent: Decimal::new(6, 0),
                loan_balance_at_sale: Decimal::ZERO,
            },
            conservative_appreciation_percent: default_conservative(),
            base_appreciation_percent: default_base(),
            bull_appreciation_percent: default_bull(),
        }
    }

    #[test]
    fn produces_ordered_scenarios_with_annual_timelines() {
        let simulation = request().calculate().expect("valid scenarios");
        assert_eq!(simulation.scenarios.len(), 3);
        assert_eq!(simulation.scenarios[0].timeline.len(), 5);
        assert!(
            simulation.scenarios[0].projected_property_value
                < simulation.scenarios[1].projected_property_value
        );
        assert!(
            simulation.scenarios[1].projected_property_value
                < simulation.scenarios[2].projected_property_value
        );
    }

    #[test]
    fn rejects_unordered_scenarios() {
        let mut request = request();
        request.bull_appreciation_percent = Decimal::new(2, 0);
        assert!(request.calculate().is_err());
    }
}
