use axum::extract::{Query, State};
use axum::response::Json;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::db::queries::{categories, transactions};
use crate::error::AppResult;
use crate::services::analytics;
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
    let breakdowns = analytics::spending_by_category(&transaction_list);

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
    let all_categories = categories::list_categories(&conn)?;

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

    // Build category hierarchy
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
                if !cat_map.contains_key(&id) {
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
        }));
    }

    // Pre-compute total income/expense per top-level category (direct +
    // children) so we can sort by total and emit nodes in tree order.
    // This prevents crossing streams in the Sankey by ensuring the node
    // array order matches the visual top-to-bottom layout.
    struct TopLevelEntry<'a> {
        cat: &'a crate::models::category::Category,
        direct: i64,
        children: Vec<(&'a str, &'a str, i64)>, // (name, color, amount)
        total: i64,
    }

    let build_entries = |amount_map: &std::collections::HashMap<i64, i64>| -> Vec<TopLevelEntry> {
        let mut entries: Vec<TopLevelEntry> = Vec::new();
        for &cat_id in &top_level_ids {
            let cat = match cat_map.get(&cat_id) {
                Some(c) => c,
                None => continue,
            };
            let child_ids = children_map.get(&cat_id).cloned().unwrap_or_default();
            let direct = amount_map.get(&cat_id).copied().unwrap_or(0);

            let mut children: Vec<(&str, &str, i64)> = Vec::new();
            for &cid in &child_ids {
                let amt = amount_map.get(&cid).copied().unwrap_or(0);
                if amt == 0 {
                    continue;
                }
                if let Some(cc) = cat_map.get(&cid) {
                    children.push((&cc.name, &cc.color, amt));
                }
            }
            children.sort_by(|a, b| b.2.cmp(&a.2));

            let child_sum: i64 = children.iter().map(|(_, _, c)| *c).sum();
            let total = direct + child_sum;
            if total == 0 {
                continue;
            }
            entries.push(TopLevelEntry {
                cat,
                direct,
                children,
                total,
            });
        }
        entries.sort_by(|a, b| b.total.cmp(&a.total));
        entries
    };

    let income_entries = build_entries(&income_by_id);
    let expense_entries = build_entries(&expense_by_id);

    // Compute column depths dynamically based on whether each side has
    // hierarchy.  Explicit depths prevent nodeAlign:"justify" from
    // pushing leaf categories into the children column.
    let income_has_hierarchy = income_entries.iter().any(|e| !e.children.is_empty());
    let expense_has_hierarchy = expense_entries.iter().any(|e| !e.children.is_empty());

    let inc_child_depth: u32 = 0;
    let inc_parent_depth: u32 = if income_has_hierarchy { 1 } else { 0 };
    let budget_depth: u32 = inc_parent_depth + 1;
    let exp_parent_depth: u32 = budget_depth + 1;
    let exp_child_depth: u32 = if expense_has_hierarchy {
        exp_parent_depth + 1
    } else {
        exp_parent_depth
    };

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

    // --- Income side: children → parent → Budget ---
    // Nodes added in tree order (sorted by total desc) so that within
    // each Sankey column the vertical positions match.
    for entry in &income_entries {
        if !entry.children.is_empty() {
            for &(name, color, cents) in &entry.children {
                ensure_node(&mut nodes, &mut node_names, name, color, inc_child_depth);
                links.push(SankeyLink {
                    source: name.to_string(),
                    target: entry.cat.name.clone(),
                    value: cents as f64 / 100.0,
                });
            }
            if entry.direct > 0 {
                let other = format!("Other {}", entry.cat.name);
                ensure_node(
                    &mut nodes,
                    &mut node_names,
                    &other,
                    &entry.cat.color,
                    inc_child_depth,
                );
                links.push(SankeyLink {
                    source: other,
                    target: entry.cat.name.clone(),
                    value: entry.direct as f64 / 100.0,
                });
            }
            ensure_node(
                &mut nodes,
                &mut node_names,
                &entry.cat.name,
                &entry.cat.color,
                inc_parent_depth,
            );
            links.push(SankeyLink {
                source: entry.cat.name.clone(),
                target: budget_name.clone(),
                value: entry.total as f64 / 100.0,
            });
        } else {
            // Leaf income: same column as parents
            ensure_node(
                &mut nodes,
                &mut node_names,
                &entry.cat.name,
                &entry.cat.color,
                inc_parent_depth,
            );
            links.push(SankeyLink {
                source: entry.cat.name.clone(),
                target: budget_name.clone(),
                value: entry.direct as f64 / 100.0,
            });
        }
    }

    if uncategorized_income > 0 {
        ensure_node(
            &mut nodes,
            &mut node_names,
            "Uncategorized",
            "#6b7280",
            inc_parent_depth,
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

    // --- Expense side: Budget → parent → children ---
    for entry in &expense_entries {
        if !entry.children.is_empty() {
            ensure_node(
                &mut nodes,
                &mut node_names,
                &entry.cat.name,
                &entry.cat.color,
                exp_parent_depth,
            );
            links.push(SankeyLink {
                source: budget_name.clone(),
                target: entry.cat.name.clone(),
                value: entry.total as f64 / 100.0,
            });
            for &(name, color, cents) in &entry.children {
                ensure_node(&mut nodes, &mut node_names, name, color, exp_child_depth);
                links.push(SankeyLink {
                    source: entry.cat.name.clone(),
                    target: name.to_string(),
                    value: cents as f64 / 100.0,
                });
            }
            if entry.direct > 0 {
                let other = format!("Other {}", entry.cat.name);
                ensure_node(
                    &mut nodes,
                    &mut node_names,
                    &other,
                    &entry.cat.color,
                    exp_child_depth,
                );
                links.push(SankeyLink {
                    source: entry.cat.name.clone(),
                    target: other,
                    value: entry.direct as f64 / 100.0,
                });
            }
        } else {
            // Leaf expense: same column as parents, not pushed right
            ensure_node(
                &mut nodes,
                &mut node_names,
                &entry.cat.name,
                &entry.cat.color,
                exp_parent_depth,
            );
            links.push(SankeyLink {
                source: budget_name.clone(),
                target: entry.cat.name.clone(),
                value: entry.direct as f64 / 100.0,
            });
        }
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
            exp_parent_depth,
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

    Ok(Json(SankeyResponse { nodes, links }))
}
