use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Json};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::db::queries::transactions;
use crate::error::AppResult;
use crate::filters::Icons;
use crate::services::analytics;
use crate::state::AppState;

/// Collect a category and all its descendants into a set of IDs.
fn collect_subtree_ids(
    root_id: i64,
    children_map: &std::collections::HashMap<i64, Vec<i64>>,
) -> std::collections::HashSet<i64> {
    let mut ids = std::collections::HashSet::new();
    let mut stack = vec![root_id];
    while let Some(id) = stack.pop() {
        ids.insert(id);
        if let Some(children) = children_map.get(&id) {
            stack.extend(children);
        }
    }
    ids
}

/// Build a children map and find the Transfers subtree IDs to exclude from analytics.
fn transfers_excluded_ids(
    all_categories: &[crate::models::category::Category],
) -> std::collections::HashSet<i64> {
    let mut children_map: std::collections::HashMap<i64, Vec<i64>> =
        std::collections::HashMap::new();
    for cat in all_categories {
        if let Some(parent_id) = cat.parent_id {
            children_map.entry(parent_id).or_default().push(cat.id);
        }
    }
    if let Some(transfers) = all_categories
        .iter()
        .find(|c| c.built_in && c.name == "Transfers")
    {
        collect_subtree_ids(transfers.id, &children_map)
    } else {
        std::collections::HashSet::new()
    }
}

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

    // Exclude Transfers subtree from the flat pie chart
    let all_cats = state.cached_categories()?;
    let excluded = transfers_excluded_ids(&all_cats);
    let filtered: Vec<_> = transaction_list
        .into_iter()
        .filter(|t| {
            t.transaction
                .category_id
                .is_none_or(|id| !excluded.contains(&id))
        })
        .collect();

    let breakdowns = analytics::spending_by_category(&filtered);

    let result: Vec<CategorySpending> = breakdowns
        .into_iter()
        .map(|b| CategorySpending {
            category: b.category,
            color: b.color,
            amount_cents: b.total_cents,
            percentage: b.percentage,
        })
        .collect();

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

/// Generate all "YYYY-MM" strings for months between two "YYYY-MM-DD" date
/// strings (inclusive of the months each date falls in).
fn all_months_in_range(from_date: &str, to_date: &str) -> Vec<String> {
    use chrono::{Datelike, NaiveDate};
    let from = match NaiveDate::parse_from_str(from_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let to = match NaiveDate::parse_from_str(to_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut months = Vec::new();
    let (mut year, mut month) = (from.year(), from.month());
    let (end_year, end_month) = (to.year(), to.month());

    while (year, month) <= (end_year, end_month) {
        months.push(format!("{year:04}-{month:02}"));
        if month == 12 {
            year += 1;
            month = 1;
        } else {
            month += 1;
        }
    }

    months
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

    let from_date_str = params.from_date.clone();
    let to_date_str = params.to_date.clone();

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

    // Seed all months in the requested date range so gaps show as zero bars.
    if let (Some(from), Some(to)) = (&from_date_str, &to_date_str) {
        for month in all_months_in_range(from, to) {
            monthly_data.entry(month).or_insert((0, 0));
        }
    }

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

fn tree_node_total(node: &CategoryTreeNode) -> i64 {
    node.amount_cents.unwrap_or(0) + node.children.iter().map(tree_node_total).sum::<i64>()
}

fn build_subtree(
    cat_id: i64,
    children_map: &std::collections::HashMap<i64, Vec<i64>>,
    spending_by_id: &std::collections::HashMap<i64, i64>,
    cat_map: &std::collections::HashMap<i64, &crate::models::category::Category>,
) -> Option<CategoryTreeNode> {
    let cat = cat_map.get(&cat_id)?;
    let child_ids = children_map.get(&cat_id).cloned().unwrap_or_default();
    let direct_spending = spending_by_id.get(&cat_id).copied().unwrap_or(0);

    let mut child_nodes: Vec<CategoryTreeNode> = child_ids
        .iter()
        .filter_map(|&id| build_subtree(id, children_map, spending_by_id, cat_map))
        .collect();

    child_nodes.sort_by_key(|n| std::cmp::Reverse(tree_node_total(n)));

    let has_children_spending = !child_nodes.is_empty();

    if has_children_spending && direct_spending > 0 {
        child_nodes.push(CategoryTreeNode {
            name: format!("Other {}", cat.name),
            color: cat.color.clone(),
            amount_cents: Some(direct_spending),
            children: Vec::new(),
        });
        Some(CategoryTreeNode {
            name: cat.name.clone(),
            color: cat.color.clone(),
            amount_cents: None,
            children: child_nodes,
        })
    } else if has_children_spending {
        Some(CategoryTreeNode {
            name: cat.name.clone(),
            color: cat.color.clone(),
            amount_cents: None,
            children: child_nodes,
        })
    } else if direct_spending > 0 {
        Some(CategoryTreeNode {
            name: cat.name.clone(),
            color: cat.color.clone(),
            amount_cents: Some(direct_spending),
            children: Vec::new(),
        })
    } else {
        None
    }
}

#[derive(Debug, Serialize)]
pub struct CategoryTreeResponse {
    pub categories: Vec<CategoryTreeNode>,
    /// Earliest transaction date in the result set (YYYY-MM-DD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_date: Option<String>,
    /// Latest transaction date in the result set (YYYY-MM-DD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_date: Option<String>,
}

pub async fn spending_by_category_tree(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsParams>,
) -> AppResult<Json<CategoryTreeResponse>> {
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

    let actual_from = transaction_list
        .iter()
        .map(|t| t.transaction.date.as_str())
        .min()
        .map(String::from);
    let actual_to = transaction_list
        .iter()
        .map(|t| t.transaction.date.as_str())
        .max()
        .map(String::from);

    let all_categories = state.cached_categories()?;
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

    // Exclude Transfers subtree
    let excluded = transfers_excluded_ids(&all_categories);

    // Keep only categories with net negative amounts (expenses), negate to positive
    let spending_by_id: std::collections::HashMap<i64, i64> = totals_by_id
        .into_iter()
        .filter(|(_, v)| *v < 0)
        .filter(|(k, _)| !excluded.contains(k))
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
        if excluded.contains(&cat.id) {
            continue;
        }
        if let Some(parent_id) = cat.parent_id {
            children_map.entry(parent_id).or_default().push(cat.id);
        } else {
            top_level_ids.push(cat.id);
        }
    }

    top_level_ids.sort();

    let mut result: Vec<CategoryTreeNode> = top_level_ids
        .iter()
        .filter_map(|&id| build_subtree(id, &children_map, &spending_by_id, &cat_map))
        .collect();

    // Add uncategorized as top-level leaf
    if uncategorized_total > 0 {
        result.push(CategoryTreeNode {
            name: "Uncategorized".into(),
            color: "#6b7280".into(),
            amount_cents: Some(uncategorized_total),
            children: Vec::new(),
        });
    }

    // Sort top-level by total spending (recursive sum of all descendants)
    result.sort_by_key(|n| std::cmp::Reverse(tree_node_total(n)));

    if result.is_empty() {
        warn!("spending_by_category_tree: no spending data in selected period");
    } else {
        debug!(
            top_level_nodes = result.len(),
            "spending_by_category_tree: returning tree"
        );
    }

    Ok(Json(CategoryTreeResponse {
        categories: result,
        from_date: actual_from,
        to_date: actual_to,
    }))
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

    let from_date_str = params.from_date.clone();
    let to_date_str = params.to_date.clone();

    let filter = transactions::TransactionFilter {
        from_date: params.from_date,
        to_date: params.to_date,
        ..Default::default()
    };

    let transaction_list = transactions::list_transactions(&conn, &filter)?;
    let all_categories = state.cached_categories()?;

    let cat_map: std::collections::HashMap<i64, &crate::models::category::Category> =
        all_categories.iter().map(|c| (c.id, c)).collect();

    // Group by (category_id, month) → amount_cents
    let mut data: std::collections::HashMap<i64, std::collections::HashMap<String, i64>> =
        std::collections::HashMap::new();
    let mut all_months: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // Seed all months in the requested date range so gaps show as zero bars.
    if let (Some(from), Some(to)) = (&from_date_str, &to_date_str) {
        for month in all_months_in_range(from, to) {
            all_months.insert(month);
        }
    }

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

#[derive(Debug, Serialize)]
pub struct SankeyNode {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub depth: u32,
}

#[derive(Debug, Serialize)]
pub struct SankeyLink {
    pub source: String,
    pub target: String,
    pub value: f64,
}

#[derive(Debug, Serialize)]
pub struct SankeyResponse {
    pub nodes: Vec<SankeyNode>,
    pub links: Vec<SankeyLink>,
    /// Earliest transaction date in the result set (YYYY-MM-DD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_date: Option<String>,
    /// Latest transaction date in the result set (YYYY-MM-DD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_date: Option<String>,
}

pub async fn flow_sankey(
    State(state): State<AppState>,
    Query(params): Query<AnalyticsParams>,
) -> AppResult<Json<SankeyResponse>> {
    debug!(
        from_date = ?params.from_date,
        to_date = ?params.to_date,
        "flow_sankey: fetching data"
    );
    let conn = state.db.get()?;

    let filter = transactions::TransactionFilter {
        from_date: params.from_date,
        to_date: params.to_date,
        ..Default::default()
    };

    let transaction_list = transactions::list_transactions(&conn, &filter)?;

    // Derive actual date range from transactions (important for the "All" preset
    // where the filter dates are 1970–2099).
    let actual_from = transaction_list
        .iter()
        .map(|t| t.transaction.date.as_str())
        .min()
        .map(String::from);
    let actual_to = transaction_list
        .iter()
        .map(|t| t.transaction.date.as_str())
        .max()
        .map(String::from);

    let all_categories = state.cached_categories()?;

    let cat_map: std::collections::HashMap<i64, &crate::models::category::Category> =
        all_categories.iter().map(|c| (c.id, c)).collect();

    debug!(
        transactions = transaction_list.len(),
        categories = all_categories.len(),
        "flow_sankey: loaded raw data"
    );

    // Sum amount_cents per category
    let mut totals_by_id: std::collections::HashMap<Option<i64>, i64> =
        std::collections::HashMap::new();

    for t in &transaction_list {
        *totals_by_id.entry(t.transaction.category_id).or_insert(0) += t.transaction.amount_cents;
    }

    // Build category hierarchy, excluding Transfers subtree
    let excluded = transfers_excluded_ids(&all_categories);

    let mut children_map: std::collections::HashMap<i64, Vec<i64>> =
        std::collections::HashMap::new();
    let mut top_level_ids: Vec<i64> = Vec::new();

    for cat in &all_categories {
        if excluded.contains(&cat.id) {
            continue;
        }
        if let Some(parent_id) = cat.parent_id {
            children_map.entry(parent_id).or_default().push(cat.id);
        } else {
            top_level_ids.push(cat.id);
        }
    }
    top_level_ids.sort();

    // Classify each category's net total as income or expense
    let mut income_by_id: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut expense_by_id: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
    let mut uncategorized_income: i64 = 0;
    let mut uncategorized_expense: i64 = 0;

    for (&cat_id, &total) in &totals_by_id {
        if total == 0 {
            continue;
        }
        match cat_id {
            Some(id) => {
                if !cat_map.contains_key(&id) || excluded.contains(&id) {
                    continue;
                }
                if total > 0 {
                    income_by_id.insert(id, total);
                } else {
                    expense_by_id.insert(id, -total);
                }
            }
            None => {
                if total > 0 {
                    uncategorized_income = total;
                } else {
                    uncategorized_expense = -total;
                }
            }
        }
    }

    if income_by_id.is_empty()
        && expense_by_id.is_empty()
        && uncategorized_income == 0
        && uncategorized_expense == 0
    {
        warn!("flow_sankey: no data in selected period");
        return Ok(Json(SankeyResponse {
            nodes: Vec::new(),
            links: Vec::new(),
            from_date: None,
            to_date: None,
        }));
    }

    // Build recursive tree for each top-level category, separately for
    // income and expense.  Each node tracks its subtotal (direct +
    // descendants) and max subtree depth so we can assign Sankey columns.
    struct SankeyTreeNode<'a> {
        cat: &'a crate::models::category::Category,
        direct: i64,
        children: Vec<SankeyTreeNode<'a>>,
        subtotal: i64,
        max_subtree_depth: u32,
    }

    fn build_sankey_tree<'a>(
        cat_id: i64,
        amount_map: &std::collections::HashMap<i64, i64>,
        children_map: &std::collections::HashMap<i64, Vec<i64>>,
        cat_map: &std::collections::HashMap<i64, &'a crate::models::category::Category>,
    ) -> Option<SankeyTreeNode<'a>> {
        let cat = cat_map.get(&cat_id)?;
        let direct = amount_map.get(&cat_id).copied().unwrap_or(0);
        let child_ids = children_map.get(&cat_id).cloned().unwrap_or_default();

        let mut children: Vec<SankeyTreeNode<'a>> = child_ids
            .iter()
            .filter_map(|&id| build_sankey_tree(id, amount_map, children_map, cat_map))
            .collect();
        children.sort_by(|a, b| b.subtotal.cmp(&a.subtotal));

        let child_sum: i64 = children.iter().map(|c| c.subtotal).sum();
        let subtotal = direct + child_sum;
        if subtotal == 0 {
            return None;
        }

        let max_subtree_depth = children
            .iter()
            .map(|c| c.max_subtree_depth + 1)
            .max()
            .unwrap_or(0);

        Some(SankeyTreeNode {
            cat,
            direct,
            children,
            subtotal,
            max_subtree_depth,
        })
    }

    let mut income_trees: Vec<SankeyTreeNode> = top_level_ids
        .iter()
        .filter_map(|&id| build_sankey_tree(id, &income_by_id, &children_map, &cat_map))
        .collect();
    income_trees.sort_by(|a, b| b.subtotal.cmp(&a.subtotal));

    let mut expense_trees: Vec<SankeyTreeNode> = top_level_ids
        .iter()
        .filter_map(|&id| build_sankey_tree(id, &expense_by_id, &children_map, &cat_map))
        .collect();
    expense_trees.sort_by(|a, b| b.subtotal.cmp(&a.subtotal));

    // Detect category names that appear on both income and expense sides.
    // Without disambiguation, shared names create cycles in the DAG
    // (e.g. "Expenses" → Budget → "Expenses").
    fn collect_tree_names(node: &SankeyTreeNode, names: &mut std::collections::HashSet<String>) {
        names.insert(node.cat.name.clone());
        if !node.children.is_empty() {
            for child in &node.children {
                collect_tree_names(child, names);
            }
            if node.direct > 0 {
                names.insert(format!("Other {}", node.cat.name));
            }
        }
    }

    let mut income_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut expense_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for tree in &income_trees {
        collect_tree_names(tree, &mut income_names);
    }
    for tree in &expense_trees {
        collect_tree_names(tree, &mut expense_names);
    }
    let overlap: std::collections::HashSet<String> =
        income_names.intersection(&expense_names).cloned().collect();

    // Column layout based on actual max depth on each side:
    //   Income leaves (col 0) ... income roots (col max_inc)
    //   | Budget (col max_inc+1) |
    //   expense roots (col max_inc+2) ... expense leaves (col max_inc+2+max_exp)
    let max_income_depth = income_trees
        .iter()
        .map(|t| t.max_subtree_depth)
        .max()
        .unwrap_or(0);
    let income_root_col = max_income_depth;
    let budget_depth = income_root_col + 1;
    let expense_root_col = budget_depth + 1;

    let budget_name = "Budget".to_string();
    let mut nodes: Vec<SankeyNode> = Vec::new();
    let mut links: Vec<SankeyLink> = Vec::new();
    let mut node_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    fn ensure_node(
        nodes: &mut Vec<SankeyNode>,
        seen: &mut std::collections::HashSet<String>,
        name: &str,
        color: &str,
        depth: u32,
    ) {
        if seen.insert(name.to_string()) {
            nodes.push(SankeyNode {
                name: name.to_string(),
                color: Some(color.to_string()),
                depth,
            });
        }
    }

    // Recursively emit income nodes.  Income flows left-to-right:
    // deepest leaves at col 0 → parents → roots → Budget.
    // A node at tree_depth d gets Sankey col = max_depth - d.
    // Names in `overlap` are suffixed with " (In)" to avoid cycles.
    fn emit_income(
        node: &SankeyTreeNode,
        tree_depth: u32,
        max_depth: u32,
        nodes: &mut Vec<SankeyNode>,
        links: &mut Vec<SankeyLink>,
        seen: &mut std::collections::HashSet<String>,
        overlap: &std::collections::HashSet<String>,
    ) {
        let col = max_depth - tree_depth;
        let name = if overlap.contains(&node.cat.name) {
            format!("{} (In)", node.cat.name)
        } else {
            node.cat.name.clone()
        };
        ensure_node(nodes, seen, &name, &node.cat.color, col);

        if !node.children.is_empty() {
            for child in &node.children {
                emit_income(
                    child,
                    tree_depth + 1,
                    max_depth,
                    nodes,
                    links,
                    seen,
                    overlap,
                );
                let child_name = if overlap.contains(&child.cat.name) {
                    format!("{} (In)", child.cat.name)
                } else {
                    child.cat.name.clone()
                };
                links.push(SankeyLink {
                    source: child_name,
                    target: name.clone(),
                    value: child.subtotal as f64 / 100.0,
                });
            }
            if node.direct > 0 {
                let other_base = format!("Other {}", node.cat.name);
                let other = if overlap.contains(&other_base) {
                    format!("{} (In)", other_base)
                } else {
                    other_base
                };
                let child_col = max_depth - (tree_depth + 1);
                ensure_node(nodes, seen, &other, &node.cat.color, child_col);
                links.push(SankeyLink {
                    source: other,
                    target: name.clone(),
                    value: node.direct as f64 / 100.0,
                });
            }
        }
    }

    // --- Income side ---
    for tree in &income_trees {
        emit_income(
            tree,
            0,
            income_root_col,
            &mut nodes,
            &mut links,
            &mut node_names,
            &overlap,
        );
        let root_name = if overlap.contains(&tree.cat.name) {
            format!("{} (In)", tree.cat.name)
        } else {
            tree.cat.name.clone()
        };
        links.push(SankeyLink {
            source: root_name,
            target: budget_name.clone(),
            value: tree.subtotal as f64 / 100.0,
        });
    }

    if uncategorized_income > 0 {
        ensure_node(
            &mut nodes,
            &mut node_names,
            "Uncategorized",
            "#6b7280",
            income_root_col,
        );
        links.push(SankeyLink {
            source: "Uncategorized".into(),
            target: budget_name.clone(),
            value: uncategorized_income as f64 / 100.0,
        });
    }

    ensure_node(
        &mut nodes,
        &mut node_names,
        &budget_name,
        "#3b82f6",
        budget_depth,
    );

    // Recursively emit expense nodes.  Expenses flow left-to-right:
    // Budget → roots → parents → deepest leaves.
    // A node at tree_depth d gets Sankey col = base_col + d.
    fn emit_expense(
        node: &SankeyTreeNode,
        tree_depth: u32,
        base_col: u32,
        nodes: &mut Vec<SankeyNode>,
        links: &mut Vec<SankeyLink>,
        seen: &mut std::collections::HashSet<String>,
    ) {
        let col = base_col + tree_depth;
        ensure_node(nodes, seen, &node.cat.name, &node.cat.color, col);

        if !node.children.is_empty() {
            for child in &node.children {
                emit_expense(child, tree_depth + 1, base_col, nodes, links, seen);
                links.push(SankeyLink {
                    source: node.cat.name.clone(),
                    target: child.cat.name.clone(),
                    value: child.subtotal as f64 / 100.0,
                });
            }
            if node.direct > 0 {
                let other = format!("Other {}", node.cat.name);
                ensure_node(
                    nodes,
                    seen,
                    &other,
                    &node.cat.color,
                    base_col + tree_depth + 1,
                );
                links.push(SankeyLink {
                    source: node.cat.name.clone(),
                    target: other,
                    value: node.direct as f64 / 100.0,
                });
            }
        }
    }

    // --- Expense side ---
    for tree in &expense_trees {
        links.push(SankeyLink {
            source: budget_name.clone(),
            target: tree.cat.name.clone(),
            value: tree.subtotal as f64 / 100.0,
        });
        emit_expense(
            tree,
            0,
            expense_root_col,
            &mut nodes,
            &mut links,
            &mut node_names,
        );
    }

    if uncategorized_expense > 0 {
        let name = if node_names.contains("Uncategorized") {
            "Uncategorized Expenses"
        } else {
            "Uncategorized"
        };
        ensure_node(
            &mut nodes,
            &mut node_names,
            name,
            "#6b7280",
            expense_root_col,
        );
        links.push(SankeyLink {
            source: budget_name.clone(),
            target: name.into(),
            value: uncategorized_expense as f64 / 100.0,
        });
    }

    debug!(
        nodes = nodes.len(),
        links = links.len(),
        "flow_sankey: returning hierarchical data"
    );

    Ok(Json(SankeyResponse {
        nodes,
        links,
        from_date: actual_from,
        to_date: actual_to,
    }))
}

// --- Icon API ---

const ICON_CACHE: &str = "public, max-age=86400, immutable";

/// Return all available icon names as a JSON array.
pub async fn icon_names() -> impl IntoResponse {
    ([(header::CACHE_CONTROL, ICON_CACHE)], Json(Icons::names()))
}

/// Return all icons as a JSON object mapping name → SVG markup.
pub async fn icon_all() -> impl IntoResponse {
    let map: std::collections::HashMap<&str, &str> = Icons::all().into_iter().collect();
    ([(header::CACHE_CONTROL, ICON_CACHE)], Json(map))
}

/// Return the inline SVG markup for a single icon by name.
pub async fn icon_svg(Path(name): Path<String>) -> impl IntoResponse {
    match Icons::svg(&name) {
        Some(svg) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "image/svg+xml"),
                (header::CACHE_CONTROL, ICON_CACHE),
            ],
            svg,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
