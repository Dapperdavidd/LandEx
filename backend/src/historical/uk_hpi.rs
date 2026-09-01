use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UkHpiRow {
    #[serde(rename = "Date", deserialize_with = "deserialize_date")]
    pub observed_on: NaiveDate,
    #[serde(rename = "RegionName")]
    pub region_name: String,
    #[serde(rename = "AreaCode")]
    pub area_code: String,
    #[serde(rename = "AveragePrice")]
    pub average_price: Decimal,
    #[serde(rename = "Index")]
    pub index: Decimal,
    #[serde(
        rename = "12m%Change",
        deserialize_with = "deserialize_optional_decimal"
    )]
    pub annual_change_percent: Option<Decimal>,
    #[serde(rename = "SalesVolume", deserialize_with = "deserialize_optional_i32")]
    pub sales_volume: Option<i32>,
}

impl UkHpiRow {
    pub fn validate(&self) -> Result<(), String> {
        if self.region_name.trim().is_empty() || self.area_code.trim().is_empty() {
            return Err("region name and area code are required".into());
        }
        if self.average_price <= Decimal::ZERO || self.index <= Decimal::ZERO {
            return Err("average price and index must be positive".into());
        }
        Ok(())
    }
}

fn deserialize_date<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    NaiveDate::parse_from_str(value.trim(), "%d/%m/%Y").map_err(serde::de::Error::custom)
}

fn deserialize_optional_decimal<'de, D>(deserializer: D) -> Result<Option<Decimal>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.trim().is_empty() {
        return Ok(None);
    }
    value
        .trim()
        .parse()
        .map(Some)
        .map_err(serde::de::Error::custom)
}

fn deserialize_optional_i32<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    if value.trim().is_empty() {
        return Ok(None);
    }
    value
        .trim()
        .parse()
        .map(Some)
        .map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_official_hpi_row() {
        let csv = "Date,RegionName,AreaCode,AveragePrice,Index,12m%Change,SalesVolume\n01/06/2026,London,E12000007,565567,130.2,1.7,1520\n";
        let row: UkHpiRow = csv::Reader::from_reader(csv.as_bytes())
            .deserialize()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(row.region_name, "London");
        assert_eq!(
            row.observed_on,
            NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()
        );
        assert_eq!(row.annual_change_percent, Some(Decimal::new(17, 1)));
        row.validate().unwrap();
    }
}
