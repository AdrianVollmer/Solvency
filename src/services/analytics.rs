use crate::models::TransactionWithRelations;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SpendingSummary {
    pub total_cents: i64,
    pub transaction_count: usize,
    pub average_cents: i64,
    pub max_transaction_cents: i64,
    pub min_transaction_cents: i64,
}

impl SpendingSummary {
    pub fn from_transactions(transactions: &[TransactionWithRelations]) -> Self {
        if transactions.is_empty() {
            return Self {
                total_cents: 0,
                transaction_count: 0,
                average_cents: 0,
                max_transaction_cents: 0,
                min_transaction_cents: 0,
            };
        }

        let total_cents: i64 = transactions.iter().map(|e| e.transaction.amount_cents).sum();
        let transaction_count = transactions.len();
        let average_cents = total_cents / transaction_count as i64;
        let max_transaction_cents = transactions
            .iter()
            .map(|e| e.transaction.amount_cents)
            .max()
            .unwrap_or(0);
        let min_transaction_cents = transactions
            .iter()
            .map(|e| e.transaction.amount_cents)
            .min()
            .unwrap_or(0);

        Self {
            total_cents,
            transaction_count,
            average_cents,
            max_transaction_cents,
            min_transaction_cents,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CategoryBreakdown {
    pub category: String,
    pub color: String,
    pub total_cents: i64,
    pub percentage: f64,
    pub transaction_count: usize,
}

pub fn spending_by_category(transactions: &[TransactionWithRelations]) -> Vec<CategoryBreakdown> {
    let mut category_data: HashMap<String, (String, i64, usize)> = HashMap::new();

    for transaction in transactions {
        let category = transaction
            .category_name
            .clone()
            .unwrap_or_else(|| "Uncategorized".into());
        let color = transaction
            .category_color
            .clone()
            .unwrap_or_else(|| "#6b7280".into());

        let entry = category_data.entry(category).or_insert((color, 0, 0));
        entry.1 += transaction.transaction.amount_cents;
        entry.2 += 1;
    }

    let total: i64 = category_data.values().map(|(_, total, _)| total).sum();

    let mut result: Vec<CategoryBreakdown> = category_data
        .into_iter()
        .map(
            |(category, (color, total_cents, transaction_count))| CategoryBreakdown {
                category,
                color,
                total_cents,
                percentage: if total > 0 {
                    (total_cents as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
                transaction_count,
            },
        )
        .collect();

    result.sort_by(|a, b| b.total_cents.cmp(&a.total_cents));
    result
}

#[derive(Debug, Clone)]
pub struct DailySpending {
    pub date: String,
    pub total_cents: i64,
    pub transaction_count: usize,
}

pub fn spending_by_day(transactions: &[TransactionWithRelations]) -> Vec<DailySpending> {
    let mut daily_data: HashMap<String, (i64, usize)> = HashMap::new();

    for transaction in transactions {
        let entry = daily_data
            .entry(transaction.transaction.date.clone())
            .or_insert((0, 0));
        entry.0 += transaction.transaction.amount_cents;
        entry.1 += 1;
    }

    let mut result: Vec<DailySpending> = daily_data
        .into_iter()
        .map(|(date, (total_cents, transaction_count))| DailySpending {
            date,
            total_cents,
            transaction_count,
        })
        .collect();

    result.sort_by(|a, b| a.date.cmp(&b.date));
    result
}

pub fn format_cents(cents: i64) -> String {
    let is_negative = cents < 0;
    let abs_cents = cents.abs();
    let dollars = abs_cents / 100;
    let remainder = abs_cents % 100;

    if is_negative {
        format!("-{}.{:02}", dollars, remainder)
    } else {
        format!("{}.{:02}", dollars, remainder)
    }
}
