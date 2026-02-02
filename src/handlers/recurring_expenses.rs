use std::collections::HashMap;

use askama::Template;
use axum::extract::State;
use axum::response::Html;
use chrono::NaiveDate;

use crate::db::queries::transactions;
use crate::error::{AppResult, RenderHtml};
use crate::filters;
use crate::models::Settings;
use crate::state::{AppState, JsManifest};
use crate::VERSION;

/// Frequency classification for recurring expenses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frequency {
    Weekly,
    Monthly,
    Quarterly,
    Yearly,
}

impl Frequency {
    fn label(self) -> &'static str {
        match self {
            Frequency::Weekly => "Weekly",
            Frequency::Monthly => "Monthly",
            Frequency::Quarterly => "Quarterly",
            Frequency::Yearly => "Yearly",
        }
    }

    fn annual_multiplier(self) -> i64 {
        match self {
            Frequency::Weekly => 52,
            Frequency::Monthly => 12,
            Frequency::Quarterly => 4,
            Frequency::Yearly => 1,
        }
    }

    fn sort_order(self) -> u8 {
        match self {
            Frequency::Weekly => 1,
            Frequency::Monthly => 2,
            Frequency::Quarterly => 3,
            Frequency::Yearly => 4,
        }
    }
}

/// Classify a median day-interval into a frequency bucket.
fn classify_interval(median_days: i64) -> Option<Frequency> {
    match median_days {
        5..=9 => Some(Frequency::Weekly),
        28..=35 => Some(Frequency::Monthly),
        85..=100 => Some(Frequency::Quarterly),
        350..=380 => Some(Frequency::Yearly),
        _ => None,
    }
}

/// A detected recurring expense ready for display.
#[derive(Clone)]
pub struct RecurringExpense {
    pub description: String,
    pub frequency_label: String,
    /// Sort order for frequency (1=weekly, 2=monthly, 3=quarterly, 4=yearly).
    pub frequency_sort: u8,
    pub typical_amount_cents: i64,
    pub typical_amount_formatted: String,
    pub last_date: String,
    pub annual_cost_cents: i64,
    pub annual_cost_formatted: String,
    pub total_spent_cents: i64,
    pub total_spent_formatted: String,
    pub occurrence_count: usize,
    /// True when the last occurrence is more than 365 days ago.
    pub inactive: bool,
}

#[derive(Template)]
#[template(path = "pages/recurring_expenses.html")]
pub struct RecurringExpensesTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: crate::filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub expenses: Vec<RecurringExpense>,
    pub inactive_expenses: Vec<RecurringExpense>,
    pub total_annual_cost_formatted: String,
    pub total_monthly_cost_formatted: String,
    pub subscription_count: usize,
}

pub async fn index(State(state): State<AppState>) -> AppResult<Html<String>> {
    let app_settings = state.load_settings()?;
    let all_expenses = state.cached_recurring_expenses()?;

    let (inactive_expenses, expenses): (Vec<_>, Vec<_>) =
        all_expenses.into_iter().partition(|e| e.inactive);

    // Summary stats reflect only active subscriptions
    let total_annual: i64 = expenses.iter().map(|e| e.annual_cost_cents).sum();
    let total_monthly = total_annual / 12;

    let template = RecurringExpensesTemplate {
        title: "Recurring Expenses".into(),
        settings: app_settings.clone(),
        icons: crate::filters::Icons,
        manifest: state.manifest.clone(),
        version: VERSION,
        xsrf_token: state.xsrf_token.value().to_string(),
        subscription_count: expenses.len(),
        expenses,
        inactive_expenses,
        total_annual_cost_formatted: filters::format_money_neutral(
            total_annual,
            &app_settings.currency,
            &app_settings.locale,
        ),
        total_monthly_cost_formatted: filters::format_money_neutral(
            total_monthly,
            &app_settings.currency,
            &app_settings.locale,
        ),
    };

    template.render_html()
}

// ---------------------------------------------------------------------------
// Detection logic
// ---------------------------------------------------------------------------

/// Normalize a description for grouping: lowercase, strip non-alphanumeric,
/// trim, and remove trailing numbers (invoice numbers, dates).
fn normalize_description(desc: &str) -> String {
    let normalized: String = desc
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    let trimmed = normalized.trim();
    trimmed
        .trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ')
        .trim()
        .to_string()
}

/// Compute the grouping key for a transaction.
/// Priority: IBAN > payee > normalized description.
fn grouping_key(iban: &Option<String>, payee: &Option<String>, description: &str) -> String {
    if let Some(ref iban) = iban {
        let trimmed = iban.trim();
        if !trimmed.is_empty() {
            return format!("iban:{}", trimmed.to_uppercase());
        }
    }
    if let Some(ref payee) = payee {
        let trimmed = payee.trim();
        if !trimmed.is_empty() {
            return format!("payee:{}", normalize_description(trimmed));
        }
    }
    format!("desc:{}", normalize_description(description))
}

struct GroupEntry {
    date: NaiveDate,
    amount_cents: i64,
    display_name: String,
}

/// Merge groups whose normalized keys share a common prefix.
/// For example, "desc:spotify" and "desc:spotifysweden" get merged under
/// the shorter key.  Only applies to non-IBAN keys with a minimum length.
fn merge_prefix_groups(
    groups: HashMap<String, Vec<GroupEntry>>,
) -> HashMap<String, Vec<GroupEntry>> {
    // Separate IBAN groups (never merged) from text groups
    let mut iban_groups: HashMap<String, Vec<GroupEntry>> = HashMap::new();
    let mut text_groups: HashMap<String, Vec<GroupEntry>> = HashMap::new();

    for (key, entries) in groups {
        if key.starts_with("iban:") {
            iban_groups.insert(key, entries);
        } else {
            text_groups.insert(key, entries);
        }
    }

    // Sort text keys by length (shortest first) for prefix merging
    let mut keys: Vec<String> = text_groups.keys().cloned().collect();
    keys.sort_by_key(|k| k.len());

    // Map from original key -> canonical (merged) key
    let mut canonical: HashMap<String, String> = HashMap::new();
    for key in &keys {
        canonical.insert(key.clone(), key.clone());
    }

    // Extract the raw suffix after "desc:" or "payee:"
    fn raw_key(k: &str) -> &str {
        k.split_once(':').map_or(k, |(_, rest)| rest)
    }

    const MIN_PREFIX_LEN: usize = 5;

    for i in 0..keys.len() {
        let short_raw = raw_key(&keys[i]);
        if short_raw.len() < MIN_PREFIX_LEN {
            continue;
        }
        for j in (i + 1)..keys.len() {
            // Only merge keys of the same type prefix
            let short_type = keys[i].split_once(':').map(|(t, _)| t);
            let long_type = keys[j].split_once(':').map(|(t, _)| t);
            if short_type != long_type {
                continue;
            }
            let long_raw = raw_key(&keys[j]);
            if long_raw.starts_with(short_raw) {
                let canon_j = canonical[&keys[j]].clone();
                let canon_i = canonical[&keys[i]].clone();
                // Repoint j (and anything already pointing to j) to i's canonical
                for val in canonical.values_mut() {
                    if *val == canon_j {
                        *val = canon_i.clone();
                    }
                }
            }
        }
    }

    // Rebuild text groups according to canonical mapping
    let mut merged: HashMap<String, Vec<GroupEntry>> = HashMap::new();
    for (key, entries) in text_groups {
        let canon = canonical.get(&key).unwrap_or(&key).clone();
        merged.entry(canon).or_default().extend(entries);
    }

    // Re-add IBAN groups
    merged.extend(iban_groups);
    merged
}

/// Compute the median of a sorted i64 slice.
fn median(sorted: &[i64]) -> i64 {
    let n = sorted.len();
    if n == 0 {
        return 0;
    }
    if n.is_multiple_of(2) {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2
    } else {
        sorted[n / 2]
    }
}

/// Detect recurring expenses from raw transaction data.
pub(crate) fn detect_recurring_expenses(
    rows: Vec<transactions::ExpenseRow>,
    currency: &str,
    locale: &str,
    today: NaiveDate,
) -> Vec<RecurringExpense> {
    // Group by key
    let mut groups: HashMap<String, Vec<GroupEntry>> = HashMap::new();

    for row in &rows {
        let key = grouping_key(&row.counterparty_iban, &row.payee, &row.description);
        if let Ok(date) = NaiveDate::parse_from_str(&row.date, "%Y-%m-%d") {
            let display_name = row
                .payee
                .as_deref()
                .filter(|p| !p.trim().is_empty())
                .unwrap_or(&row.description)
                .to_string();
            groups.entry(key).or_default().push(GroupEntry {
                date,
                amount_cents: row.amount_cents,
                display_name,
            });
        }
    }

    // Merge groups with similar prefixes
    let groups = merge_prefix_groups(groups);

    let mut results = Vec::new();

    for (_key, mut entries) in groups {
        if entries.len() < 3 {
            continue;
        }

        entries.sort_by_key(|e| e.date);

        // Compute median absolute amount
        let mut sorted_amounts: Vec<i64> = entries.iter().map(|e| e.amount_cents.abs()).collect();
        sorted_amounts.sort();
        let median_amount = median(&sorted_amounts);

        // Filter entries within 5% tolerance of median amount
        let tolerance = (median_amount as f64 * 0.05).max(100.0) as i64;
        let filtered: Vec<&GroupEntry> = entries
            .iter()
            .filter(|e| (e.amount_cents.abs() - median_amount).abs() <= tolerance)
            .collect();

        if filtered.len() < 3 {
            continue;
        }

        // Compute intervals between consecutive dates
        let dates: Vec<NaiveDate> = filtered.iter().map(|e| e.date).collect();
        let mut intervals: Vec<i64> = dates.windows(2).map(|w| (w[1] - w[0]).num_days()).collect();

        if intervals.is_empty() {
            continue;
        }

        intervals.sort();
        let median_interval = median(&intervals);

        if let Some(frequency) = classify_interval(median_interval) {
            let total_spent: i64 = filtered.iter().map(|e| e.amount_cents.abs()).sum();
            let annual_cost = median_amount * frequency.annual_multiplier();
            let description = filtered.last().unwrap().display_name.clone();
            let last_date = *dates.last().unwrap();
            let inactive = (today - last_date).num_days() > 365;

            results.push(RecurringExpense {
                description,
                frequency_label: frequency.label().to_string(),
                frequency_sort: frequency.sort_order(),
                typical_amount_cents: median_amount,
                typical_amount_formatted: filters::format_money_neutral(
                    median_amount,
                    currency,
                    locale,
                ),
                last_date: last_date.format("%Y-%m-%d").to_string(),
                annual_cost_cents: annual_cost,
                annual_cost_formatted: filters::format_money_neutral(annual_cost, currency, locale),
                total_spent_cents: total_spent,
                total_spent_formatted: filters::format_money_neutral(total_spent, currency, locale),
                occurrence_count: filtered.len(),
                inactive,
            });
        }
    }

    // Sort by estimated annual cost descending
    results.sort_by(|a, b| b.annual_cost_cents.cmp(&a.annual_cost_cents));

    results
}

/// Build a children map and find the Transfers subtree IDs to exclude.
pub(crate) fn transfers_excluded_ids(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_description() {
        assert_eq!(normalize_description("Spotify AB"), "spotify ab");
        assert_eq!(normalize_description("NETFLIX.COM 12345"), "netflixcom");
        assert_eq!(normalize_description("  Hello World  "), "hello world");
    }

    #[test]
    fn test_classify_interval() {
        assert_eq!(classify_interval(7), Some(Frequency::Weekly));
        assert_eq!(classify_interval(30), Some(Frequency::Monthly));
        assert_eq!(classify_interval(91), Some(Frequency::Quarterly));
        assert_eq!(classify_interval(365), Some(Frequency::Yearly));
        assert_eq!(classify_interval(15), None);
        assert_eq!(classify_interval(200), None);
    }

    #[test]
    fn test_median() {
        assert_eq!(median(&[1, 2, 3]), 2);
        assert_eq!(median(&[1, 2, 3, 4]), 2);
        assert_eq!(median(&[10, 20, 30, 40, 50]), 30);
        assert_eq!(median(&[]), 0);
    }

    #[test]
    fn test_grouping_key_prefers_iban() {
        let key = grouping_key(
            &Some("DE89370400440532013000".to_string()),
            &Some("Payee".to_string()),
            "Description",
        );
        assert!(key.starts_with("iban:"));
    }

    #[test]
    fn test_grouping_key_falls_back_to_payee() {
        let key = grouping_key(&None, &Some("Netflix".to_string()), "NETFLIX.COM");
        assert!(key.starts_with("payee:"));
    }

    #[test]
    fn test_grouping_key_falls_back_to_description() {
        let key = grouping_key(&None, &None, "NETFLIX.COM");
        assert!(key.starts_with("desc:"));
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2024, 12, 1).unwrap()
    }

    #[test]
    fn test_detect_monthly_subscription() {
        let rows: Vec<transactions::ExpenseRow> = (0..6)
            .map(|i| transactions::ExpenseRow {
                date: format!("2024-{:02}-15", i + 1),
                amount_cents: -999,
                description: "Spotify AB".to_string(),
                payee: None,
                counterparty_iban: None,
            })
            .collect();

        let results = detect_recurring_expenses(rows, "EUR", "en-US", today());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].frequency_label, "Monthly");
        assert_eq!(results[0].occurrence_count, 6);
        assert!(!results[0].inactive);
    }

    #[test]
    fn test_inactive_subscription() {
        // Last occurrence 2023-06-15, which is >365 days before 2024-12-01
        let rows: Vec<transactions::ExpenseRow> = (0..6)
            .map(|i| transactions::ExpenseRow {
                date: format!("2023-{:02}-15", i + 1),
                amount_cents: -999,
                description: "Old Service".to_string(),
                payee: None,
                counterparty_iban: None,
            })
            .collect();

        let results = detect_recurring_expenses(rows, "EUR", "en-US", today());
        assert_eq!(results.len(), 1);
        assert!(results[0].inactive);
    }

    #[test]
    fn test_too_few_occurrences_not_detected() {
        let rows = vec![
            transactions::ExpenseRow {
                date: "2024-01-15".to_string(),
                amount_cents: -999,
                description: "One-off".to_string(),
                payee: None,
                counterparty_iban: None,
            },
            transactions::ExpenseRow {
                date: "2024-02-15".to_string(),
                amount_cents: -999,
                description: "One-off".to_string(),
                payee: None,
                counterparty_iban: None,
            },
        ];

        let results = detect_recurring_expenses(rows, "EUR", "en-US", today());
        assert!(results.is_empty());
    }

    #[test]
    fn test_irregular_intervals_not_detected() {
        // Intervals: 14 days, 7 days -> median 10 -> no bucket match
        let rows = vec![
            transactions::ExpenseRow {
                date: "2024-01-01".to_string(),
                amount_cents: -500,
                description: "Random".to_string(),
                payee: None,
                counterparty_iban: None,
            },
            transactions::ExpenseRow {
                date: "2024-01-15".to_string(),
                amount_cents: -500,
                description: "Random".to_string(),
                payee: None,
                counterparty_iban: None,
            },
            transactions::ExpenseRow {
                date: "2024-01-22".to_string(),
                amount_cents: -500,
                description: "Random".to_string(),
                payee: None,
                counterparty_iban: None,
            },
        ];

        let results = detect_recurring_expenses(rows, "EUR", "en-US", today());
        assert!(results.is_empty());
    }

    #[test]
    fn test_prefix_merge_groups() {
        let mut groups: HashMap<String, Vec<GroupEntry>> = HashMap::new();
        let mk = |d: &str| GroupEntry {
            date: NaiveDate::parse_from_str(d, "%Y-%m-%d").unwrap(),
            amount_cents: -999,
            display_name: "test".to_string(),
        };
        groups.insert(
            "desc:spotify".to_string(),
            vec![mk("2024-01-01"), mk("2024-02-01")],
        );
        groups.insert(
            "desc:spotifysweden".to_string(),
            vec![mk("2024-03-01"), mk("2024-04-01")],
        );

        let merged = merge_prefix_groups(groups);
        // Should be merged into one group
        assert_eq!(merged.len(), 1);
        let entries = merged.values().next().unwrap();
        assert_eq!(entries.len(), 4);
    }
}
