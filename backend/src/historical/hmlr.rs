use std::str::FromStr;

use chrono::NaiveDate;
use csv::StringRecord;
use rust_decimal::Decimal;
use thiserror::Error;
use uuid::Uuid;

const FIELD_COUNT: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub struct PricePaidTransaction {
    pub transaction_id: Uuid,
    pub price: Decimal,
    pub transferred_on: NaiveDate,
    pub postcode: Option<String>,
    pub property_type: char,
    pub new_build: bool,
    pub tenure: char,
    pub paon: Option<String>,
    pub saon: Option<String>,
    pub street: Option<String>,
    pub locality: Option<String>,
    pub town_city: String,
    pub district: Option<String>,
    pub county: Option<String>,
    pub ppd_category: char,
    pub record_status: char,
}

impl PricePaidTransaction {
    pub fn from_record(record: &StringRecord) -> Result<Self, PricePaidParseError> {
        if record.len() != FIELD_COUNT {
            return Err(PricePaidParseError::FieldCount(record.len()));
        }
        let field = |index: usize| record.get(index).unwrap_or_default().trim();
        let code = |index: usize| {
            field(index)
                .chars()
                .next()
                .ok_or(PricePaidParseError::MissingField(index))
        };
        let transaction_id = Uuid::parse_str(field(0).trim_matches(|c| c == '{' || c == '}'))?;
        let price = Decimal::from_str(field(1))?;
        if price <= Decimal::ZERO {
            return Err(PricePaidParseError::InvalidPrice);
        }
        let transferred_on = NaiveDate::parse_from_str(field(2), "%Y-%m-%d %H:%M")?;
        let property_type = code(4)?;
        if !matches!(property_type, 'D' | 'S' | 'T' | 'F' | 'O') {
            return Err(PricePaidParseError::InvalidCode(
                "property_type",
                property_type,
            ));
        }
        let new_build = match code(5)? {
            'Y' => true,
            'N' => false,
            value => return Err(PricePaidParseError::InvalidCode("new_build", value)),
        };
        let tenure = code(6)?;
        if !matches!(tenure, 'F' | 'L' | 'U') {
            return Err(PricePaidParseError::InvalidCode("tenure", tenure));
        }
        let ppd_category = code(14)?;
        if !matches!(ppd_category, 'A' | 'B') {
            return Err(PricePaidParseError::InvalidCode(
                "ppd_category",
                ppd_category,
            ));
        }
        let record_status = code(15)?;
        if !matches!(record_status, 'A' | 'C' | 'D') {
            return Err(PricePaidParseError::InvalidCode(
                "record_status",
                record_status,
            ));
        }
        let town_city = field(11).to_owned();
        if town_city.is_empty() {
            return Err(PricePaidParseError::MissingField(11));
        }
        Ok(Self {
            transaction_id,
            price,
            transferred_on,
            postcode: optional(field(3)),
            property_type,
            new_build,
            tenure,
            paon: optional(field(7)),
            saon: optional(field(8)),
            street: optional(field(9)),
            locality: optional(field(10)),
            town_city,
            district: optional(field(12)),
            county: optional(field(13)),
            ppd_category,
            record_status,
        })
    }
}

fn optional(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

#[derive(Debug, Error)]
pub enum PricePaidParseError {
    #[error("expected 16 fields, received {0}")]
    FieldCount(usize),
    #[error("required field {0} is empty")]
    MissingField(usize),
    #[error("price must be positive")]
    InvalidPrice,
    #[error("invalid {0} code: {1}")]
    InvalidCode(&'static str, char),
    #[error(transparent)]
    Uuid(#[from] uuid::Error),
    #[error(transparent)]
    Decimal(#[from] rust_decimal::Error),
    #[error(transparent)]
    Date(#[from] chrono::ParseError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_official_price_paid_row() {
        let row = StringRecord::from(vec![
            "{A1E6F85A-7D47-4C7B-8A6A-000000000001}",
            "720000",
            "2025-04-18 00:00",
            "E14 9GE",
            "F",
            "N",
            "L",
            "2408",
            "",
            "MARINA WALK",
            "",
            "LONDON",
            "TOWER HAMLETS",
            "GREATER LONDON",
            "A",
            "A",
        ]);
        let parsed = PricePaidTransaction::from_record(&row).unwrap();
        assert_eq!(parsed.price, Decimal::new(720000, 0));
        assert_eq!(
            parsed.transferred_on,
            NaiveDate::from_ymd_opt(2025, 4, 18).unwrap()
        );
        assert_eq!(parsed.postcode.as_deref(), Some("E14 9GE"));
        assert_eq!(parsed.town_city, "LONDON");
    }

    #[test]
    fn rejects_an_unknown_property_type() {
        let row = StringRecord::from(vec![
            "{A1E6F85A-7D47-4C7B-8A6A-000000000001}",
            "720000",
            "2025-04-18 00:00",
            "E14 9GE",
            "X",
            "N",
            "L",
            "2408",
            "",
            "MARINA WALK",
            "",
            "LONDON",
            "TOWER HAMLETS",
            "GREATER LONDON",
            "A",
            "A",
        ]);
        assert!(matches!(
            PricePaidTransaction::from_record(&row),
            Err(PricePaidParseError::InvalidCode("property_type", 'X'))
        ));
    }

    #[test]
    fn preserves_an_unclassified_historical_tenure() {
        let row = StringRecord::from(vec![
            "{A1E6F85A-7D47-4C7B-8A6A-000000000002}",
            "95000",
            "1995-01-03 00:00",
            "",
            "O",
            "N",
            "U",
            "1",
            "",
            "HIGH STREET",
            "",
            "LONDON",
            "",
            "GREATER LONDON",
            "A",
            "A",
        ]);
        assert_eq!(PricePaidTransaction::from_record(&row).unwrap().tenure, 'U');
    }
}
