use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedExpense {
    pub date: String,
    pub amount: String,
    pub currency: String,
    pub description: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub row_number: usize,
}

impl ParsedExpense {
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
    pub expenses: Vec<ParsedExpense>,
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

    let date_col = find_column(&headers, &["date", "Date", "DATE", "transaction_date"]);
    let amount_col = find_column(&headers, &["amount", "Amount", "AMOUNT", "value", "Value"]);
    let desc_col = find_column(
        &headers,
        &[
            "description",
            "Description",
            "DESCRIPTION",
            "memo",
            "Memo",
            "note",
            "Note",
        ],
    );
    let currency_col = find_column(&headers, &["currency", "Currency", "CURRENCY"]);
    let category_col = find_column(&headers, &["category", "Category", "CATEGORY"]);
    let tags_col = find_column(&headers, &["tags", "Tags", "TAGS", "labels", "Labels"]);

    let date_col =
        date_col.ok_or_else(|| AppError::CsvParse("No date column found in CSV".into()))?;
    let amount_col =
        amount_col.ok_or_else(|| AppError::CsvParse("No amount column found in CSV".into()))?;
    let desc_col =
        desc_col.ok_or_else(|| AppError::CsvParse("No description column found in CSV".into()))?;

    let mut expenses = Vec::new();
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

        let tags: Vec<String> = tags_col
            .and_then(|col| record.get(col))
            .map(|s| {
                s.split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        expenses.push(ParsedExpense {
            date,
            amount: amount_clean,
            currency,
            description,
            category,
            tags,
            row_number,
        });
    }

    Ok(ParseResult { expenses, errors })
}

fn find_column(headers: &csv::StringRecord, candidates: &[&str]) -> Option<usize> {
    for candidate in candidates {
        for (idx, header) in headers.iter().enumerate() {
            if header.trim().eq_ignore_ascii_case(candidate) {
                return Some(idx);
            }
        }
    }
    None
}

fn clean_amount(amount: &str) -> String {
    let mut result = String::new();
    let mut has_decimal = false;

    for c in amount.chars() {
        if c.is_ascii_digit() {
            result.push(c);
        } else if (c == '.' || c == ',') && !has_decimal {
            result.push('.');
            has_decimal = true;
        } else if c == '-' && result.is_empty() {
            result.push(c);
        }
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
        assert_eq!(result.expenses.len(), 2);
        assert_eq!(result.errors.len(), 0);

        assert_eq!(result.expenses[0].date, "2024-01-15");
        assert_eq!(result.expenses[0].amount, "50.00");
        assert_eq!(result.expenses[0].description, "Groceries");
    }

    #[test]
    fn test_clean_amount() {
        assert_eq!(clean_amount("$50.00"), "50.00");
        assert_eq!(clean_amount("-$25.50"), "-25.50");
        assert_eq!(clean_amount("1,234.56"), "1234.56");
        assert_eq!(clean_amount("€100"), "100");
    }
}
