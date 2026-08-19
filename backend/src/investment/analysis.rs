use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MONTHS_PER_YEAR: Decimal = Decimal::from_parts(12, 0, 0, false, 0);
const ONE_HUNDRED: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

#[derive(Clone, Debug, Deserialize)]
pub struct InvestmentInputs {
    pub purchase_price: Decimal,
    pub monthly_rent: Decimal,
    #[serde(default)]
    pub annual_other_income: Decimal,
    #[serde(default)]
    pub vacancy_rate_percent: Decimal,
    #[serde(default)]
    pub annual_property_tax: Decimal,
    #[serde(default)]
    pub annual_insurance: Decimal,
    #[serde(default)]
    pub annual_maintenance: Decimal,
    #[serde(default)]
    pub annual_management: Decimal,
    #[serde(default)]
    pub annual_other_expenses: Decimal,
    #[serde(default)]
    pub annual_debt_service: Decimal,
    pub down_payment: Option<Decimal>,
    #[serde(default)]
    pub closing_costs: Decimal,
    #[serde(default)]
    pub annual_appreciation_percent: Decimal,
    pub holding_years: u16,
    #[serde(default)]
    pub sale_cost_percent: Decimal,
    #[serde(default)]
    pub loan_balance_at_sale: Decimal,
}

impl InvestmentInputs {
    pub fn calculate(&self) -> Result<InvestmentAnalysis, CalculationError> {
        self.validate()?;

        let annual_rent = checked_mul(self.monthly_rent, MONTHS_PER_YEAR)?;
        let gross_annual_income = checked_add(annual_rent, self.annual_other_income)?;
        let vacancy_multiplier = checked_sub(
            Decimal::ONE,
            checked_div(self.vacancy_rate_percent, ONE_HUNDRED)?,
        )?;
        let effective_annual_income = checked_mul(gross_annual_income, vacancy_multiplier)?;
        let operating_expenses = [
            self.annual_property_tax,
            self.annual_insurance,
            self.annual_maintenance,
            self.annual_management,
            self.annual_other_expenses,
        ]
        .into_iter()
        .try_fold(Decimal::ZERO, checked_add)?;
        let net_operating_income = checked_sub(effective_annual_income, operating_expenses)?;
        let annual_cash_flow = checked_sub(net_operating_income, self.annual_debt_service)?;
        let cash_invested = checked_add(
            self.down_payment.unwrap_or(self.purchase_price),
            self.closing_costs,
        )?;

        let growth_rate = checked_add(
            Decimal::ONE,
            checked_div(self.annual_appreciation_percent, ONE_HUNDRED)?,
        )?;
        let mut projected_property_value = self.purchase_price;
        for _ in 0..self.holding_years {
            projected_property_value = checked_mul(projected_property_value, growth_rate)?;
        }

        let sale_cost = checked_mul(
            projected_property_value,
            checked_div(self.sale_cost_percent, ONE_HUNDRED)?,
        )?;
        let net_sale_proceeds = checked_sub(
            checked_sub(projected_property_value, sale_cost)?,
            self.loan_balance_at_sale,
        )?;
        let cumulative_cash_flow =
            checked_mul(annual_cash_flow, Decimal::from(self.holding_years))?;
        let total_profit = checked_sub(
            checked_add(net_sale_proceeds, cumulative_cash_flow)?,
            cash_invested,
        )?;

        Ok(InvestmentAnalysis {
            annual_rent: rounded(annual_rent),
            gross_annual_income: rounded(gross_annual_income),
            effective_annual_income: rounded(effective_annual_income),
            annual_operating_expenses: rounded(operating_expenses),
            net_operating_income: rounded(net_operating_income),
            annual_cash_flow: rounded(annual_cash_flow),
            cash_invested: rounded(cash_invested),
            gross_rental_yield_percent: ratio_percent(annual_rent, self.purchase_price)?,
            cap_rate_percent: ratio_percent(net_operating_income, self.purchase_price)?,
            cash_on_cash_return_percent: ratio_percent(annual_cash_flow, cash_invested)?,
            projected_property_value: rounded(projected_property_value),
            projected_net_sale_proceeds: rounded(net_sale_proceeds),
            projected_cumulative_cash_flow: rounded(cumulative_cash_flow),
            projected_total_profit: rounded(total_profit),
            projected_total_return_percent: ratio_percent(total_profit, cash_invested)?,
            holding_years: self.holding_years,
        })
    }

    fn validate(&self) -> Result<(), CalculationError> {
        if self.purchase_price <= Decimal::ZERO {
            return Err(CalculationError::InvalidInput(
                "purchase_price must be greater than zero".to_owned(),
            ));
        }
        if self.holding_years == 0 || self.holding_years > 100 {
            return Err(CalculationError::InvalidInput(
                "holding_years must be between 1 and 100".to_owned(),
            ));
        }
        if self.vacancy_rate_percent < Decimal::ZERO || self.vacancy_rate_percent > ONE_HUNDRED {
            return Err(CalculationError::InvalidInput(
                "vacancy_rate_percent must be between 0 and 100".to_owned(),
            ));
        }
        if self.sale_cost_percent < Decimal::ZERO || self.sale_cost_percent > ONE_HUNDRED {
            return Err(CalculationError::InvalidInput(
                "sale_cost_percent must be between 0 and 100".to_owned(),
            ));
        }
        if self.annual_appreciation_percent <= -ONE_HUNDRED {
            return Err(CalculationError::InvalidInput(
                "annual_appreciation_percent must be greater than -100".to_owned(),
            ));
        }

        let non_negative_values = [
            ("monthly_rent", self.monthly_rent),
            ("annual_other_income", self.annual_other_income),
            ("annual_property_tax", self.annual_property_tax),
            ("annual_insurance", self.annual_insurance),
            ("annual_maintenance", self.annual_maintenance),
            ("annual_management", self.annual_management),
            ("annual_other_expenses", self.annual_other_expenses),
            ("annual_debt_service", self.annual_debt_service),
            ("closing_costs", self.closing_costs),
            ("loan_balance_at_sale", self.loan_balance_at_sale),
        ];
        if let Some((name, _)) = non_negative_values
            .into_iter()
            .find(|(_, value)| *value < Decimal::ZERO)
        {
            return Err(CalculationError::InvalidInput(format!(
                "{name} cannot be negative"
            )));
        }

        if let Some(down_payment) = self.down_payment
            && (down_payment <= Decimal::ZERO || down_payment > self.purchase_price)
        {
            return Err(CalculationError::InvalidInput(
                "down_payment must be greater than zero and no more than purchase_price".to_owned(),
            ));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InvestmentAnalysis {
    pub annual_rent: Decimal,
    pub gross_annual_income: Decimal,
    pub effective_annual_income: Decimal,
    pub annual_operating_expenses: Decimal,
    pub net_operating_income: Decimal,
    pub annual_cash_flow: Decimal,
    pub cash_invested: Decimal,
    pub gross_rental_yield_percent: Decimal,
    pub cap_rate_percent: Decimal,
    pub cash_on_cash_return_percent: Decimal,
    pub projected_property_value: Decimal,
    pub projected_net_sale_proceeds: Decimal,
    pub projected_cumulative_cash_flow: Decimal,
    pub projected_total_profit: Decimal,
    pub projected_total_return_percent: Decimal,
    pub holding_years: u16,
}

#[derive(Debug, Error)]
pub enum CalculationError {
    #[error("{0}")]
    InvalidInput(String),
    #[error("calculation exceeded the supported numeric range")]
    Overflow,
}

fn ratio_percent(numerator: Decimal, denominator: Decimal) -> Result<Decimal, CalculationError> {
    Ok(rounded(checked_mul(
        checked_div(numerator, denominator)?,
        ONE_HUNDRED,
    )?))
}

fn checked_add(left: Decimal, right: Decimal) -> Result<Decimal, CalculationError> {
    left.checked_add(right).ok_or(CalculationError::Overflow)
}

fn checked_sub(left: Decimal, right: Decimal) -> Result<Decimal, CalculationError> {
    left.checked_sub(right).ok_or(CalculationError::Overflow)
}

fn checked_mul(left: Decimal, right: Decimal) -> Result<Decimal, CalculationError> {
    left.checked_mul(right).ok_or(CalculationError::Overflow)
}

fn checked_div(left: Decimal, right: Decimal) -> Result<Decimal, CalculationError> {
    left.checked_div(right).ok_or(CalculationError::Overflow)
}

fn rounded(value: Decimal) -> Decimal {
    value.round_dp(4)
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::InvestmentInputs;

    #[test]
    fn calculates_unlevered_investment_metrics() {
        let analysis = InvestmentInputs {
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
            annual_appreciation_percent: Decimal::new(5, 0),
            holding_years: 5,
            sale_cost_percent: Decimal::new(6, 0),
            loan_balance_at_sale: Decimal::ZERO,
        }
        .calculate()
        .expect("valid analysis");

        assert_eq!(analysis.annual_rent, Decimal::new(42_000, 0));
        assert_eq!(analysis.gross_rental_yield_percent, Decimal::new(84, 1));
        assert_eq!(analysis.net_operating_income, Decimal::new(24_900, 0));
        assert_eq!(analysis.cap_rate_percent, Decimal::new(498, 2));
        assert!(analysis.projected_total_profit > Decimal::ZERO);
    }

    #[test]
    fn rejects_impossible_vacancy_rate() {
        let result = InvestmentInputs {
            purchase_price: Decimal::new(100_000, 0),
            monthly_rent: Decimal::new(1_000, 0),
            annual_other_income: Decimal::ZERO,
            vacancy_rate_percent: Decimal::new(101, 0),
            annual_property_tax: Decimal::ZERO,
            annual_insurance: Decimal::ZERO,
            annual_maintenance: Decimal::ZERO,
            annual_management: Decimal::ZERO,
            annual_other_expenses: Decimal::ZERO,
            annual_debt_service: Decimal::ZERO,
            down_payment: None,
            closing_costs: Decimal::ZERO,
            annual_appreciation_percent: Decimal::ZERO,
            holding_years: 5,
            sale_cost_percent: Decimal::ZERO,
            loan_balance_at_sale: Decimal::ZERO,
        }
        .calculate();

        assert!(result.is_err());
    }
}
