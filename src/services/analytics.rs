use crate::models::ExpenseWithRelations;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SpendingSummary {
    pub total_cents: i64,
    pub expense_count: usize,
    pub average_cents: i64,
    pub max_expense_cents: i64,
    pub min_expense_cents: i64,
}

impl SpendingSummary {
    pub fn from_expenses(expenses: &[ExpenseWithRelations]) -> Self {
        if expenses.is_empty() {
            return Self {
                total_cents: 0,
                expense_count: 0,
                average_cents: 0,
                max_expense_cents: 0,
                min_expense_cents: 0,
            };
        }

        let total_cents: i64 = expenses.iter().map(|e| e.expense.amount_cents).sum();
        let expense_count = expenses.len();
        let average_cents = total_cents / expense_count as i64;
        let max_expense_cents = expenses
            .iter()
            .map(|e| e.expense.amount_cents)
            .max()
            .unwrap_or(0);
        let min_expense_cents = expenses
            .iter()
            .map(|e| e.expense.amount_cents)
            .min()
            .unwrap_or(0);

        Self {
            total_cents,
            expense_count,
            average_cents,
            max_expense_cents,
            min_expense_cents,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CategoryBreakdown {
    pub category: String,
    pub color: String,
    pub total_cents: i64,
    pub percentage: f64,
    pub expense_count: usize,
}

pub fn spending_by_category(expenses: &[ExpenseWithRelations]) -> Vec<CategoryBreakdown> {
    let mut category_data: HashMap<String, (String, i64, usize)> = HashMap::new();

    for expense in expenses {
        let category = expense
            .category_name
            .clone()
            .unwrap_or_else(|| "Uncategorized".into());
        let color = expense
            .category_color
            .clone()
            .unwrap_or_else(|| "#6b7280".into());

        let entry = category_data.entry(category).or_insert((color, 0, 0));
        entry.1 += expense.expense.amount_cents;
        entry.2 += 1;
    }

    let total: i64 = category_data.values().map(|(_, total, _)| total).sum();

    let mut result: Vec<CategoryBreakdown> = category_data
        .into_iter()
        .map(
            |(category, (color, total_cents, expense_count))| CategoryBreakdown {
                category,
                color,
                total_cents,
                percentage: if total > 0 {
                    (total_cents as f64 / total as f64) * 100.0
                } else {
                    0.0
                },
                expense_count,
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
    pub expense_count: usize,
}

pub fn spending_by_day(expenses: &[ExpenseWithRelations]) -> Vec<DailySpending> {
    let mut daily_data: HashMap<String, (i64, usize)> = HashMap::new();

    for expense in expenses {
        let entry = daily_data
            .entry(expense.expense.date.clone())
            .or_insert((0, 0));
        entry.0 += expense.expense.amount_cents;
        entry.1 += 1;
    }

    let mut result: Vec<DailySpending> = daily_data
        .into_iter()
        .map(|(date, (total_cents, expense_count))| DailySpending {
            date,
            total_cents,
            expense_count,
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
