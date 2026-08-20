use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::analysis::CalculationError;

const DAYS_PER_YEAR: Decimal = Decimal::from_parts(365, 0, 0, false, 0);
const ONE_HUNDRED: Decimal = Decimal::from_parts(100, 0, 0, false, 0);

#[derive(Clone, Debug, Deserialize)]
pub struct ShortletInvestmentInputs {
    pub purchase_price: Decimal,
    pub nightly_rate: Decimal,
    pub occupancy_rate_percent: Decimal,
    #[serde(default)]
    pub annual_other_income: Decimal,
    #[serde(default)]
    pub platform_fee_percent: Decimal,
    #[serde(default)]
    pub variable_cost_per_occupied_night: Decimal,
    #[serde(default)]
    pub annual_fixed_operating_expenses: Decimal,
    #[serde(default)]
    pub annual_debt_service: Decimal,
    pub down_payment: Option<Decimal>,
    #[serde(default)]
    pub closing_costs: Decimal,
}

impl ShortletInvestmentInputs {
    pub fn calculate(&self) -> Result<ShortletInvestmentAnalysis, CalculationError> {
        self.validate()?;

        let occupancy = div(self.occupancy_rate_percent, ONE_HUNDRED)?;
        let occupied_nights = mul(DAYS_PER_YEAR, occupancy)?;
        let gross_booking_revenue = mul(self.nightly_rate, occupied_nights)?;
        let platform_fees = mul(
            gross_booking_revenue,
            div(self.platform_fee_percent, ONE_HUNDRED)?,
        )?;
        let variable_operating_expenses =
            mul(self.variable_cost_per_occupied_night, occupied_nights)?;
        let gross_annual_income = add(gross_booking_revenue, self.annual_other_income)?;
        let annual_operating_expenses = add(
            add(platform_fees, variable_operating_expenses)?,
            self.annual_fixed_operating_expenses,
        )?;
        let net_operating_income = sub(gross_annual_income, annual_operating_expenses)?;
        let annual_cash_flow = sub(net_operating_income, self.annual_debt_service)?;
        let cash_invested = add(
            self.down_payment.unwrap_or(self.purchase_price),
            self.closing_costs,
        )?;

        let fee_multiplier = sub(Decimal::ONE, div(self.platform_fee_percent, ONE_HUNDRED)?)?;
        let contribution_per_occupied_night = sub(
            mul(self.nightly_rate, fee_multiplier)?,
            self.variable_cost_per_occupied_night,
        )?;
        let annual_costs_to_cover = add(
            self.annual_fixed_operating_expenses,
            self.annual_debt_service,
        )?;
        let break_even_occupancy_rate_percent = ratio_percent(
            annual_costs_to_cover,
            mul(DAYS_PER_YEAR, contribution_per_occupied_night)?,
        )?;

        Ok(ShortletInvestmentAnalysis {
            available_nights: DAYS_PER_YEAR,
            occupied_nights: rounded(occupied_nights),
            gross_booking_revenue: rounded(gross_booking_revenue),
            gross_annual_income: rounded(gross_annual_income),
            platform_fees: rounded(platform_fees),
            variable_operating_expenses: rounded(variable_operating_expenses),
            annual_operating_expenses: rounded(annual_operating_expenses),
            net_operating_income: rounded(net_operating_income),
            annual_cash_flow: rounded(annual_cash_flow),
            cash_invested: rounded(cash_invested),
            gross_booking_yield_percent: ratio_percent(gross_booking_revenue, self.purchase_price)?,
            cap_rate_percent: ratio_percent(net_operating_income, self.purchase_price)?,
            cash_on_cash_return_percent: ratio_percent(annual_cash_flow, cash_invested)?,
            break_even_occupancy_rate_percent,
        })
    }

    fn validate(&self) -> Result<(), CalculationError> {
        if self.purchase_price <= Decimal::ZERO {
            return invalid("purchase_price must be greater than zero");
        }
        if self.nightly_rate <= Decimal::ZERO {
            return invalid("nightly_rate must be greater than zero");
        }
        if !(Decimal::ZERO..=ONE_HUNDRED).contains(&self.occupancy_rate_percent) {
            return invalid("occupancy_rate_percent must be between 0 and 100");
        }
        if !(Decimal::ZERO..ONE_HUNDRED).contains(&self.platform_fee_percent) {
            return invalid("platform_fee_percent must be between 0 and less than 100");
        }

        for (name, value) in [
            ("annual_other_income", self.annual_other_income),
            (
                "variable_cost_per_occupied_night",
                self.variable_cost_per_occupied_night,
            ),
            (
                "annual_fixed_operating_expenses",
                self.annual_fixed_operating_expenses,
            ),
            ("annual_debt_service", self.annual_debt_service),
            ("closing_costs", self.closing_costs),
        ] {
            if value < Decimal::ZERO {
                return invalid(&format!("{name} cannot be negative"));
            }
        }

        if let Some(down_payment) = self.down_payment
            && (down_payment <= Decimal::ZERO || down_payment > self.purchase_price)
        {
            return invalid(
                "down_payment must be greater than zero and no more than purchase_price",
            );
        }

        let fee_multiplier = sub(Decimal::ONE, div(self.platform_fee_percent, ONE_HUNDRED)?)?;
        if mul(self.nightly_rate, fee_multiplier)? <= self.variable_cost_per_occupied_night {
            return invalid(
                "nightly revenue after platform fees must exceed variable cost per occupied night",
            );
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ShortletInvestmentAnalysis {
    pub available_nights: Decimal,
    pub occupied_nights: Decimal,
    pub gross_booking_revenue: Decimal,
    pub gross_annual_income: Decimal,
    pub platform_fees: Decimal,
    pub variable_operating_expenses: Decimal,
    pub annual_operating_expenses: Decimal,
    pub net_operating_income: Decimal,
    pub annual_cash_flow: Decimal,
    pub cash_invested: Decimal,
    pub gross_booking_yield_percent: Decimal,
    pub cap_rate_percent: Decimal,
    pub cash_on_cash_return_percent: Decimal,
    pub break_even_occupancy_rate_percent: Decimal,
}

fn invalid<T>(message: &str) -> Result<T, CalculationError> {
    Err(CalculationError::InvalidInput(message.to_owned()))
}

fn add(left: Decimal, right: Decimal) -> Result<Decimal, CalculationError> {
    left.checked_add(right).ok_or(CalculationError::Overflow)
}

fn sub(left: Decimal, right: Decimal) -> Result<Decimal, CalculationError> {
    left.checked_sub(right).ok_or(CalculationError::Overflow)
}

fn mul(left: Decimal, right: Decimal) -> Result<Decimal, CalculationError> {
    left.checked_mul(right).ok_or(CalculationError::Overflow)
}

fn div(left: Decimal, right: Decimal) -> Result<Decimal, CalculationError> {
    left.checked_div(right).ok_or(CalculationError::Overflow)
}

fn ratio_percent(numerator: Decimal, denominator: Decimal) -> Result<Decimal, CalculationError> {
    Ok(rounded(mul(div(numerator, denominator)?, ONE_HUNDRED)?))
}

fn rounded(value: Decimal) -> Decimal {
    value.round_dp(4)
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use super::ShortletInvestmentInputs;

    fn inputs() -> ShortletInvestmentInputs {
        ShortletInvestmentInputs {
            purchase_price: Decimal::new(50_000_000, 0),
            nightly_rate: Decimal::new(100_000, 0),
            occupancy_rate_percent: Decimal::new(60, 0),
            annual_other_income: Decimal::ZERO,
            platform_fee_percent: Decimal::new(10, 0),
            variable_cost_per_occupied_night: Decimal::new(15_000, 0),
            annual_fixed_operating_expenses: Decimal::new(3_000_000, 0),
            annual_debt_service: Decimal::ZERO,
            down_payment: None,
            closing_costs: Decimal::ZERO,
        }
    }

    #[test]
    fn calculates_shortlet_metrics_from_nightly_pricing() {
        let analysis = inputs().calculate().expect("valid shortlet analysis");

        assert_eq!(analysis.occupied_nights, Decimal::new(219, 0));
        assert_eq!(analysis.gross_booking_revenue, Decimal::new(21_900_000, 0));
        assert_eq!(analysis.platform_fees, Decimal::new(2_190_000, 0));
        assert_eq!(
            analysis.variable_operating_expenses,
            Decimal::new(3_285_000, 0)
        );
        assert_eq!(analysis.net_operating_income, Decimal::new(13_425_000, 0));
        assert_eq!(analysis.cap_rate_percent, Decimal::new(2685, 2));
        assert_eq!(
            analysis.break_even_occupancy_rate_percent,
            Decimal::new(109589, 4)
        );
    }

    #[test]
    fn rejects_costs_that_eliminate_per_night_contribution() {
        let mut request = inputs();
        request.variable_cost_per_occupied_night = Decimal::new(90_000, 0);

        assert!(request.calculate().is_err());
    }
}
