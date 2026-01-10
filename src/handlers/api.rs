use axum::extract::{Query, State};
use axum::response::Json;
use serde::{Deserialize, Serialize};

use crate::db::queries::expenses;
use crate::error::AppResult;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AnalyticsParams {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CategorySpending {
    pub category: String,
    pub color: String,
    pub amount_cents: i64,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct TimeSeriesPoint {
    pub date: String,
    pub amount_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct MonthlySummary {
    pub month: String,
    pub total_cents: i64,
    pub expense_count: i64,
    pub average_cents: i64,
}

pub async fn spending_by_category(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsParams>,
) -> AppResult<Json<Vec<CategorySpending>>> {
    let conn = state.db.get()?;

    let filter = expenses::ExpenseFilter {
        from_date: params.from_date,
        to_date: params.to_date,
        ..Default::default()
    };

    let expense_list = expenses::list_expenses(&conn, &filter)?;

    let mut category_totals: std::collections::HashMap<String, (String, i64)> =
        std::collections::HashMap::new();

    for expense in &expense_list {
        let category_name = expense
            .category_name
            .clone()
            .unwrap_or_else(|| "Uncategorized".into());
        let color = expense
            .category_color
            .clone()
            .unwrap_or_else(|| "#6b7280".into());

        let entry = category_totals
            .entry(category_name)
            .or_insert((color, 0));
        entry.1 += expense.expense.amount_cents;
    }

    let total: i64 = category_totals.values().map(|(_, v)| v).sum();

    let mut result: Vec<CategorySpending> = category_totals
        .into_iter()
        .map(|(category, (color, amount_cents))| CategorySpending {
            category,
            color,
            amount_cents,
            percentage: if total > 0 {
                (amount_cents as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        })
        .collect();

    result.sort_by(|a, b| b.amount_cents.cmp(&a.amount_cents));

    Ok(Json(result))
}

pub async fn spending_over_time(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsParams>,
) -> AppResult<Json<Vec<TimeSeriesPoint>>> {
    let conn = state.db.get()?;

    let filter = expenses::ExpenseFilter {
        from_date: params.from_date,
        to_date: params.to_date,
        ..Default::default()
    };

    let expense_list = expenses::list_expenses(&conn, &filter)?;

    let mut daily_totals: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();

    for expense in &expense_list {
        let entry = daily_totals
            .entry(expense.expense.date.clone())
            .or_insert(0);
        *entry += expense.expense.amount_cents;
    }

    let mut result: Vec<TimeSeriesPoint> = daily_totals
        .into_iter()
        .map(|(date, amount_cents)| TimeSeriesPoint { date, amount_cents })
        .collect();

    result.sort_by(|a, b| a.date.cmp(&b.date));

    Ok(Json(result))
}

pub async fn monthly_summary(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsParams>,
) -> AppResult<Json<Vec<MonthlySummary>>> {
    let conn = state.db.get()?;

    let filter = expenses::ExpenseFilter {
        from_date: params.from_date,
        to_date: params.to_date,
        ..Default::default()
    };

    let expense_list = expenses::list_expenses(&conn, &filter)?;

    let mut monthly_data: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();

    for expense in &expense_list {
        let month = if expense.expense.date.len() >= 7 {
            expense.expense.date[..7].to_string()
        } else {
            continue;
        };

        let entry = monthly_data.entry(month).or_insert((0, 0));
        entry.0 += expense.expense.amount_cents;
        entry.1 += 1;
    }

    let mut result: Vec<MonthlySummary> = monthly_data
        .into_iter()
        .map(|(month, (total_cents, expense_count))| MonthlySummary {
            month,
            total_cents,
            expense_count,
            average_cents: if expense_count > 0 {
                total_cents / expense_count
            } else {
                0
            },
        })
        .collect();

    result.sort_by(|a, b| a.month.cmp(&b.month));

    Ok(Json(result))
}
