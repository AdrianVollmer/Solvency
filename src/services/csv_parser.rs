use crate::error::AppError;
use serde::{Deserialize, Serialize};
use tracing::{debug, trace, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedTransaction {
    pub date: String,
    pub amount: String,
    pub currency: String,
    pub description: String,
    pub category: Option<String>,
    pub account_id: Option<i64>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub value_date: Option<String>,
    pub payer: Option<String>,
    pub payee: Option<String>,
    pub reference: Option<String>,
    pub transaction_type: Option<String>,
    pub counterparty_iban: Option<String>,
    pub creditor_id: Option<String>,
    pub mandate_reference: Option<String>,
    pub customer_reference: Option<String>,
    pub row_number: usize,
}

impl ParsedTransaction {
    pub fn tags_joined(&self) -> String {
        self.tags.join(", ")
    }

    pub fn has_category(&self) -> bool {
        self.category.is_some()
    }

    pub fn category_matches(&self, name: &str) -> bool {
        self.category.as_ref().map(|c| c == name).unwrap_or(false)
    }
}

#[derive(Debug)]
pub struct ParseResult {
    pub transactions: Vec<ParsedTransaction>,
    pub errors: Vec<String>,
}

pub fn parse_csv(content: &[u8]) -> Result<ParseResult, AppError> {
    trace!(content_size = content.len(), "Starting CSV parsing");

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

    debug!(column_count = headers.len(), "CSV headers parsed");

    // Required columns
    let date_col = find_column(&headers, "date");
    let amount_col = find_column(&headers, "amount");
    let desc_col = find_column(&headers, "description");

    // Optional columns
    let currency_col = find_column(&headers, "currency");
    let category_col = find_column(&headers, "category");
    let account_id_col = find_column(&headers, "account_id");
    let tags_col = find_column(&headers, "tags");
    let notes_col = find_column(&headers, "notes");
    let value_date_col = find_column(&headers, "value_date");
    let payer_col = find_column(&headers, "payer");
    let payee_col = find_column(&headers, "payee");
    let reference_col = find_column(&headers, "reference");
    let transaction_type_col = find_column(&headers, "transaction_type");
    let counterparty_iban_col = find_column(&headers, "counterparty_iban");
    let creditor_id_col = find_column(&headers, "creditor_id");
    let mandate_reference_col = find_column(&headers, "mandate_reference");
    let customer_reference_col = find_column(&headers, "customer_reference");

    let date_col =
        date_col.ok_or_else(|| AppError::CsvParse("No date column found in CSV".into()))?;
    let amount_col =
        amount_col.ok_or_else(|| AppError::CsvParse("No amount column found in CSV".into()))?;
    let desc_col =
        desc_col.ok_or_else(|| AppError::CsvParse("No description column found in CSV".into()))?;

    let mut transactions = Vec::new();
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
        let amount = record.get(amount_col).unwrap_or("").trim().to_string();
        let description = record.get(desc_col).unwrap_or("").trim().to_string();

        if date.is_empty() || amount.is_empty() {
            errors.push(format!("Row {}: Missing date or amount", row_number));
            continue;
        }

        let amount_clean = clean_amount(&amount);
        if amount_clean.parse::<f64>().is_err() {
            errors.push(format!("Row {}: Invalid amount '{}'", row_number, amount));
            continue;
        }

        let currency = currency_col
            .and_then(|col| record.get(col))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "USD".to_string());

        let category = category_col
            .and_then(|col| record.get(col))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let account_id = account_id_col
            .and_then(|col| record.get(col))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .and_then(|s| s.parse::<i64>().ok());

        let tags: Vec<String> = tags_col
            .and_then(|col| record.get(col))
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        let notes = get_optional_field(&record, notes_col);
        let value_date = get_optional_field(&record, value_date_col);
        let payer = get_optional_field(&record, payer_col);
        let payee = get_optional_field(&record, payee_col);
        let reference = get_optional_field(&record, reference_col);
        let transaction_type = get_optional_field(&record, transaction_type_col);
        let counterparty_iban = get_optional_field(&record, counterparty_iban_col);
        let creditor_id = get_optional_field(&record, creditor_id_col);
        let mandate_reference = get_optional_field(&record, mandate_reference_col);
        let customer_reference = get_optional_field(&record, customer_reference_col);

        transactions.push(ParsedTransaction {
            date,
            amount: amount_clean,
            currency,
            description,
            category,
            account_id,
            tags,
            notes,
            value_date,
            payer,
            payee,
            reference,
            transaction_type,
            counterparty_iban,
            creditor_id,
            mandate_reference,
            customer_reference,
            row_number,
        });
    }

    if !errors.is_empty() {
        warn!(
            error_count = errors.len(),
            "CSV parsing completed with errors"
        );
    }
    debug!(
        row_count = transactions.len(),
        error_count = errors.len(),
        "CSV parsing completed"
    );

    Ok(ParseResult {
        transactions,
        errors,
    })
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
        let csv = b"date,amount,description\n2024-01-15,50.00,Groceries\n2024-01-16,25.50,Coffee";

        let result = parse_csv(csv).unwrap();
        assert_eq!(result.transactions.len(), 2);
        assert_eq!(result.errors.len(), 0);

        assert_eq!(result.transactions[0].date, "2024-01-15");
        assert_eq!(result.transactions[0].amount, "50.00");
        assert_eq!(result.transactions[0].description, "Groceries");
    }

    #[test]
    fn test_clean_amount() {
        assert_eq!(clean_amount("$50.00"), "50.00");
        assert_eq!(clean_amount("-$25.50"), "-25.50");
        assert_eq!(clean_amount("1,234.56"), "1234.56");
        assert_eq!(clean_amount("€100"), "100");
    }
}
