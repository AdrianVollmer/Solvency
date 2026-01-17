use crate::error::AppError;
use crate::models::TradingActivityType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTradingActivity {
    pub date: String,
    pub symbol: String,
    pub quantity: Option<String>,
    pub activity_type: String,
    pub unit_price: Option<String>,
    pub currency: String,
    pub fee: Option<String>,
    pub row_number: usize,
}

impl ParsedTradingActivity {
    pub fn activity_type_label(&self) -> &str {
        self.activity_type
            .parse::<TradingActivityType>()
            .map(|t| t.label())
            .unwrap_or(&self.activity_type)
    }

    pub fn quantity_display(&self) -> String {
        self.quantity.clone().unwrap_or_default()
    }

    pub fn unit_price_display(&self) -> String {
        self.unit_price.clone().unwrap_or_default()
    }

    pub fn fee_display(&self) -> String {
        self.fee.clone().unwrap_or_else(|| "0".to_string())
    }
}

#[derive(Debug)]
pub struct ParseResult {
    pub activities: Vec<ParsedTradingActivity>,
    pub errors: Vec<String>,
}

pub fn parse_csv(content: &[u8]) -> Result<ParseResult, AppError> {
    let content_str =
        std::str::from_utf8(content).map_err(|e| AppError::CsvParse(e.to_string()))?;

    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content_str.as_bytes());

    let headers = reader
        .headers()
        .map_err(|e| AppError::CsvParse(e.to_string()))?
        .clone();

    // Required columns
    let date_col = find_column(&headers, "date");
    let symbol_col = find_column(&headers, "symbol");
    let activity_type_col = find_column(&headers, "activityType")
        .or_else(|| find_column(&headers, "activity_type"))
        .or_else(|| find_column(&headers, "type"));

    // Optional columns
    let quantity_col = find_column(&headers, "quantity");
    let unit_price_col = find_column(&headers, "unitPrice")
        .or_else(|| find_column(&headers, "unit_price"))
        .or_else(|| find_column(&headers, "price"));
    let currency_col = find_column(&headers, "currency");
    let fee_col = find_column(&headers, "fee");

    let date_col =
        date_col.ok_or_else(|| AppError::CsvParse("No date column found in CSV".into()))?;
    let symbol_col =
        symbol_col.ok_or_else(|| AppError::CsvParse("No symbol column found in CSV".into()))?;
    let activity_type_col = activity_type_col
        .ok_or_else(|| AppError::CsvParse("No activityType column found in CSV".into()))?;

    let mut activities = Vec::new();
    let mut errors = Vec::new();

    for (row_idx, result) in reader.records().enumerate() {
        let row_number = row_idx + 2;

        let record = match result {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("Row {}: {}", row_number, e));
                continue;
            }
        };

        let date = record.get(date_col).unwrap_or("").trim().to_string();
        let symbol = record.get(symbol_col).unwrap_or("").trim().to_string();
        let activity_type = record
            .get(activity_type_col)
            .unwrap_or("")
            .trim()
            .to_uppercase();

        if date.is_empty() {
            errors.push(format!("Row {}: Missing date", row_number));
            continue;
        }

        if symbol.is_empty() {
            errors.push(format!("Row {}: Missing symbol", row_number));
            continue;
        }

        if activity_type.is_empty() {
            errors.push(format!("Row {}: Missing activity type", row_number));
            continue;
        }

        // Validate activity type
        if activity_type.parse::<TradingActivityType>().is_err() {
            errors.push(format!(
                "Row {}: Invalid activity type '{}'",
                row_number, activity_type
            ));
            continue;
        }

        let quantity = get_optional_field(&record, quantity_col).map(|q| clean_amount(&q));
        let unit_price = get_optional_field(&record, unit_price_col).map(|p| clean_amount(&p));
        let fee = get_optional_field(&record, fee_col).map(|f| clean_amount(&f));

        // Validate numeric fields if present
        if let Some(ref q) = quantity {
            if q.parse::<f64>().is_err() {
                errors.push(format!(
                    "Row {}: Invalid quantity '{}'",
                    row_number,
                    record.get(quantity_col.unwrap()).unwrap_or("")
                ));
                continue;
            }
        }

        if let Some(ref p) = unit_price {
            if p.parse::<f64>().is_err() {
                errors.push(format!(
                    "Row {}: Invalid unit price '{}'",
                    row_number,
                    record.get(unit_price_col.unwrap()).unwrap_or("")
                ));
                continue;
            }
        }

        if let Some(ref f) = fee {
            if f.parse::<f64>().is_err() {
                errors.push(format!(
                    "Row {}: Invalid fee '{}'",
                    row_number,
                    record.get(fee_col.unwrap()).unwrap_or("")
                ));
                continue;
            }
        }

        let currency = currency_col
            .and_then(|col| record.get(col))
            .map(|s| s.trim().to_uppercase())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "USD".to_string());

        activities.push(ParsedTradingActivity {
            date,
            symbol,
            quantity,
            activity_type,
            unit_price,
            currency,
            fee,
            row_number,
        });
    }

    Ok(ParseResult { activities, errors })
}

fn find_column(headers: &csv::StringRecord, name: &str) -> Option<usize> {
    headers
        .iter()
        .position(|header| header.trim().eq_ignore_ascii_case(name))
}

fn get_optional_field(record: &csv::StringRecord, col: Option<usize>) -> Option<String> {
    col.and_then(|c| record.get(c))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn clean_amount(amount: &str) -> String {
    // Determine which character is the decimal separator:
    // If both . and , appear, the last one is the decimal separator
    let last_dot = amount.rfind('.');
    let last_comma = amount.rfind(',');

    let decimal_char = match (last_dot, last_comma) {
        (Some(d), Some(c)) => {
            if d > c {
                Some('.')
            } else {
                Some(',')
            }
        }
        (Some(_), None) => Some('.'),
        (None, Some(_)) => Some(','),
        (None, None) => None,
    };

    let mut result = String::new();
    let mut has_decimal = false;

    for c in amount.chars() {
        if c.is_ascii_digit() {
            result.push(c);
        } else if Some(c) == decimal_char && !has_decimal {
            result.push('.');
            has_decimal = true;
        } else if c == '-' && result.is_empty() {
            result.push(c);
        }
        // Skip thousand separators and currency symbols
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_csv() {
        let csv = b"date,symbol,activityType,quantity,unitPrice,currency,fee\n2024-01-15,AAPL,BUY,10,150.00,USD,5.00\n2024-01-16,$CASH-USD,DEPOSIT,,,USD,";

        let result = parse_csv(csv).unwrap();
        assert_eq!(result.activities.len(), 2);
        assert_eq!(result.errors.len(), 0);

        assert_eq!(result.activities[0].date, "2024-01-15");
        assert_eq!(result.activities[0].symbol, "AAPL");
        assert_eq!(result.activities[0].activity_type, "BUY");
        assert_eq!(result.activities[0].quantity, Some("10".to_string()));
        assert_eq!(result.activities[0].unit_price, Some("150.00".to_string()));

        assert_eq!(result.activities[1].symbol, "$CASH-USD");
        assert_eq!(result.activities[1].activity_type, "DEPOSIT");
    }

    #[test]
    fn test_clean_amount() {
        assert_eq!(clean_amount("$50.00"), "50.00");
        assert_eq!(clean_amount("-$25.50"), "-25.50");
        assert_eq!(clean_amount("1,234.56"), "1234.56");
        assert_eq!(clean_amount("100"), "100");
    }
}
