use axum::extract::{Query, State};
use axum::response::Json;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::db::queries::{categories, transactions};
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
    pub transaction_count: i64,
    pub average_cents: i64,
}

pub async fn spending_by_category(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsParams>,
) -> AppResult<Json<Vec<CategorySpending>>> {
    let conn = state.db.get()?;

    let filter = transactions::TransactionFilter {
        from_date: params.from_date,
        to_date: params.to_date,
        ..Default::default()
    };

    let transaction_list = transactions::list_transactions(&conn, &filter)?;

    let mut category_totals: std::collections::HashMap<String, (String, i64)> =
        std::collections::HashMap::new();

    for transaction in &transaction_list {
        let category_name = transaction
            .category_name
            .clone()
            .unwrap_or_else(|| "Uncategorized".into());
        let color = transaction
            .category_color
            .clone()
            .unwrap_or_else(|| "#6b7280".into());

        let entry = category_totals.entry(category_name).or_insert((color, 0));
        entry.1 += transaction.transaction.amount_cents;
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
    debug!(
        from_date = ?params.from_date,
        to_date = ?params.to_date,
        "spending_over_time: fetching data"
    );
    let conn = state.db.get()?;

    let filter = transactions::TransactionFilter {
        from_date: params.from_date,
        to_date: params.to_date,
        ..Default::default()
    };

    let transaction_list = transactions::list_transactions(&conn, &filter)?;
    debug!(
        transactions = transaction_list.len(),
        "spending_over_time: loaded transactions"
    );

    let mut daily_totals: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    for transaction in &transaction_list {
        let entry = daily_totals
            .entry(transaction.transaction.date.clone())
            .or_insert(0);
        *entry += transaction.transaction.amount_cents;
    }

    let mut result: Vec<TimeSeriesPoint> = daily_totals
        .into_iter()
        .map(|(date, amount_cents)| TimeSeriesPoint { date, amount_cents })
        .collect();

    result.sort_by(|a, b| a.date.cmp(&b.date));

    if result.is_empty() {
        warn!("spending_over_time: no data points in selected period");
    } else {
        debug!(
            data_points = result.len(),
            "spending_over_time: returning time series"
        );
    }

    Ok(Json(result))
}

pub async fn monthly_summary(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsParams>,
) -> AppResult<Json<Vec<MonthlySummary>>> {
    debug!(
        from_date = ?params.from_date,
        to_date = ?params.to_date,
        "monthly_summary: fetching data"
    );
    let conn = state.db.get()?;

    let filter = transactions::TransactionFilter {
        from_date: params.from_date,
        to_date: params.to_date,
        ..Default::default()
    };

    let transaction_list = transactions::list_transactions(&conn, &filter)?;
    debug!(
        transactions = transaction_list.len(),
        "monthly_summary: loaded transactions"
    );

    let mut monthly_data: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();

    for transaction in &transaction_list {
        let month = if transaction.transaction.date.len() >= 7 {
            transaction.transaction.date[..7].to_string()
        } else {
            continue;
        };

        let entry = monthly_data.entry(month).or_insert((0, 0));
        entry.0 += transaction.transaction.amount_cents;
        entry.1 += 1;
    }

    let mut result: Vec<MonthlySummary> = monthly_data
        .into_iter()
        .map(|(month, (total_cents, transaction_count))| MonthlySummary {
            month,
            total_cents,
            transaction_count,
            average_cents: if transaction_count > 0 {
                total_cents / transaction_count
            } else {
                0
            },
        })
        .collect();

    result.sort_by(|a, b| a.month.cmp(&b.month));

    if result.is_empty() {
        warn!("monthly_summary: no monthly data in selected period");
    } else {
        debug!(
            months = result.len(),
            "monthly_summary: returning summaries"
        );
    }

    Ok(Json(result))
}

#[derive(Debug, Serialize)]
pub struct CategoryTreeNode {
    pub name: String,
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount_cents: Option<i64>,
    pub children: Vec<CategoryTreeNode>,
}

pub async fn spending_by_category_tree(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsParams>,
) -> AppResult<Json<Vec<CategoryTreeNode>>> {
    debug!(
        from_date = ?params.from_date,
        to_date = ?params.to_date,
        "spending_by_category_tree: fetching data"
    );
    let conn = state.db.get()?;

    let filter = transactions::TransactionFilter {
        from_date: params.from_date,
        to_date: params.to_date,
        ..Default::default()
    };

    let transaction_list = transactions::list_transactions(&conn, &filter)?;
    let all_categories = categories::list_categories(&conn)?;
    debug!(
        transactions = transaction_list.len(),
        categories = all_categories.len(),
        "spending_by_category_tree: loaded raw data"
    );

    // Group spending by category_id, then keep only net-negative
    // categories (actual spending) and flip sign to positive.
    let mut totals_by_id: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut uncategorized_total: i64 = 0;

    for transaction in &transaction_list {
        if let Some(cat_id) = transaction.transaction.category_id {
            *totals_by_id.entry(cat_id).or_insert(0) += transaction.transaction.amount_cents;
        } else {
            uncategorized_total += transaction.transaction.amount_cents;
        }
    }

    // Keep only categories with net negative amounts (expenses), negate to positive
    let spending_by_id: std::collections::HashMap<i64, i64> = totals_by_id
        .into_iter()
        .filter(|(_, v)| *v < 0)
        .map(|(k, v)| (k, -v))
        .collect();
    let uncategorized_total = if uncategorized_total < 0 {
        -uncategorized_total
    } else {
        0
    };

    // Build category lookup maps
    let cat_map: std::collections::HashMap<i64, &crate::models::category::Category> =
        all_categories.iter().map(|c| (c.id, c)).collect();

    // Find children for each parent
    let mut children_map: std::collections::HashMap<i64, Vec<i64>> =
        std::collections::HashMap::new();
    let mut top_level_ids: Vec<i64> = Vec::new();

    for cat in &all_categories {
        if let Some(parent_id) = cat.parent_id {
            children_map.entry(parent_id).or_default().push(cat.id);
        } else {
            top_level_ids.push(cat.id);
        }
    }

    top_level_ids.sort();

    let mut result: Vec<CategoryTreeNode> = Vec::new();

    for &cat_id in &top_level_ids {
        let cat = match cat_map.get(&cat_id) {
            Some(c) => c,
            None => continue,
        };

        let child_ids = children_map.get(&cat_id).cloned().unwrap_or_default();
        let direct_spending = spending_by_id.get(&cat_id).copied().unwrap_or(0);

        // Build children nodes (only those with spending)
        let mut child_nodes: Vec<CategoryTreeNode> = Vec::new();
        for &child_id in &child_ids {
            let child_amount = spending_by_id.get(&child_id).copied().unwrap_or(0);
            if child_amount == 0 {
                continue;
            }
            if let Some(child_cat) = cat_map.get(&child_id) {
                child_nodes.push(CategoryTreeNode {
                    name: child_cat.name.clone(),
                    color: child_cat.color.clone(),
                    amount_cents: Some(child_amount),
                    children: Vec::new(),
                });
            }
        }

        child_nodes.sort_by(|a, b| {
            b.amount_cents
                .unwrap_or(0)
                .cmp(&a.amount_cents.unwrap_or(0))
        });

        let has_children_spending = !child_nodes.is_empty();

        if has_children_spending && direct_spending > 0 {
            // Parent has both direct spending and children: add pseudo-child
            child_nodes.push(CategoryTreeNode {
                name: format!("Other {}", cat.name),
                color: cat.color.clone(),
                amount_cents: Some(direct_spending),
                children: Vec::new(),
            });
            result.push(CategoryTreeNode {
                name: cat.name.clone(),
                color: cat.color.clone(),
                amount_cents: None,
                children: child_nodes,
            });
        } else if has_children_spending {
            // Parent has only children spending
            result.push(CategoryTreeNode {
                name: cat.name.clone(),
                color: cat.color.clone(),
                amount_cents: None,
                children: child_nodes,
            });
        } else if direct_spending > 0 {
            // Leaf node with direct spending only
            result.push(CategoryTreeNode {
                name: cat.name.clone(),
                color: cat.color.clone(),
                amount_cents: Some(direct_spending),
                children: Vec::new(),
            });
        }
        // Skip categories with no spending at all
    }

    // Add uncategorized as top-level leaf
    if uncategorized_total > 0 {
        result.push(CategoryTreeNode {
            name: "Uncategorized".into(),
            color: "#6b7280".into(),
            amount_cents: Some(uncategorized_total),
            children: Vec::new(),
        });
    }

    // Sort top-level by total spending (sum of children or direct)
    result.sort_by(|a, b| {
        let total_a = if a.children.is_empty() {
            a.amount_cents.unwrap_or(0)
        } else {
            a.children.iter().map(|c| c.amount_cents.unwrap_or(0)).sum()
        };
        let total_b = if b.children.is_empty() {
            b.amount_cents.unwrap_or(0)
        } else {
            b.children.iter().map(|c| c.amount_cents.unwrap_or(0)).sum()
        };
        total_b.cmp(&total_a)
    });

    if result.is_empty() {
        warn!("spending_by_category_tree: no spending data in selected period");
    } else {
        debug!(
            top_level_nodes = result.len(),
            "spending_by_category_tree: returning tree"
        );
    }

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct MonthlyByCategoryParams {
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub category_ids: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MonthlyCategorySeries {
    pub category: String,
    pub color: String,
    pub totals: Vec<i64>,
}

#[derive(Debug, Serialize)]
pub struct MonthlyByCategoryResponse {
    pub months: Vec<String>,
    pub series: Vec<MonthlyCategorySeries>,
}

pub async fn monthly_by_category(
    State(state): State<AppState>,
    Query(params): Query<MonthlyByCategoryParams>,
) -> AppResult<Json<MonthlyByCategoryResponse>> {
    debug!(
        from_date = ?params.from_date,
        to_date = ?params.to_date,
        category_ids = ?params.category_ids,
        "monthly_by_category: fetching data"
    );
    let conn = state.db.get()?;

    let selected_ids: std::collections::HashSet<i64> = params
        .category_ids
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();

    if selected_ids.is_empty() {
        debug!("monthly_by_category: no category IDs provided, returning empty");
        return Ok(Json(MonthlyByCategoryResponse {
            months: Vec::new(),
            series: Vec::new(),
        }));
    }

    let filter = transactions::TransactionFilter {
        from_date: params.from_date,
        to_date: params.to_date,
        ..Default::default()
    };

    let transaction_list = transactions::list_transactions(&conn, &filter)?;
    let all_categories = categories::list_categories(&conn)?;

    let cat_map: std::collections::HashMap<i64, &crate::models::category::Category> =
        all_categories.iter().map(|c| (c.id, c)).collect();

    // Group by (category_id, month) → amount_cents
    let mut data: std::collections::HashMap<i64, std::collections::HashMap<String, i64>> =
        std::collections::HashMap::new();
    let mut all_months: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    for transaction in &transaction_list {
        let cat_id = match transaction.transaction.category_id {
            Some(id) if selected_ids.contains(&id) => id,
            _ => continue,
        };

        let month = if transaction.transaction.date.len() >= 7 {
            transaction.transaction.date[..7].to_string()
        } else {
            continue;
        };

        all_months.insert(month.clone());
        *data.entry(cat_id).or_default().entry(month).or_insert(0) +=
            transaction.transaction.amount_cents;
    }

    let months: Vec<String> = all_months.into_iter().collect();

    // Build series, filtering to spending (negative) and negating
    let mut series: Vec<MonthlyCategorySeries> = Vec::new();
    for &cat_id in &selected_ids {
        let cat = match cat_map.get(&cat_id) {
            Some(c) => c,
            None => continue,
        };

        let month_totals = match data.get(&cat_id) {
            Some(m) => m,
            None => continue,
        };

        let totals: Vec<i64> = months
            .iter()
            .map(|m| {
                let raw = month_totals.get(m).copied().unwrap_or(0);
                if raw < 0 {
                    -raw
                } else {
                    0
                }
            })
            .collect();

        // Skip categories with no spending at all
        if totals.iter().all(|&v| v == 0) {
            continue;
        }

        series.push(MonthlyCategorySeries {
            category: cat.name.clone(),
            color: cat.color.clone(),
            totals,
        });
    }

    // Sort series by total spending descending
    series.sort_by(|a, b| {
        let sum_a: i64 = a.totals.iter().sum();
        let sum_b: i64 = b.totals.iter().sum();
        sum_b.cmp(&sum_a)
    });

    if series.is_empty() {
        warn!(
            selected = selected_ids.len(),
            "monthly_by_category: no spending data for selected categories"
        );
    } else {
        debug!(
            months = months.len(),
            series = series.len(),
            "monthly_by_category: returning data"
        );
    }

    Ok(Json(MonthlyByCategoryResponse { months, series }))
}
