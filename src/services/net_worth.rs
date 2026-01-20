use crate::db::queries::net_worth::{
    get_all_activities_ordered, get_all_market_data, get_daily_expense_sums, get_earliest_date,
    get_last_trade_prices, get_latest_date,
};
use crate::models::net_worth::{NetWorthDataPoint, NetWorthSummary};
use crate::models::trading::TradingActivityType;
use chrono::{Duration, NaiveDate};
use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap};

/// Price lookup supporting exact date match and carry-forward
struct PriceLookup {
    /// symbol -> (date -> price_cents)
    by_symbol: HashMap<String, BTreeMap<String, i64>>,
    /// symbol -> fallback_price_cents (from last trade)
    fallback: HashMap<String, i64>,
}

impl PriceLookup {
    fn new() -> Self {
        Self {
            by_symbol: HashMap::new(),
            fallback: HashMap::new(),
        }
    }

    fn add_market_data(&mut self, symbol: &str, date: &str, price_cents: i64) {
        self.by_symbol
            .entry(symbol.to_string())
            .or_default()
            .insert(date.to_string(), price_cents);
    }

    fn set_fallback(&mut self, symbol: &str, price_cents: i64) {
        self.fallback.insert(symbol.to_string(), price_cents);
    }

    /// Get price for a symbol on a specific date, with carry-forward
    fn get_price(&self, symbol: &str, date: &str) -> Option<i64> {
        // Cash positions don't need market lookup
        if symbol.starts_with("$CASH-") {
            return None;
        }

        if let Some(prices) = self.by_symbol.get(symbol) {
            // Try exact match first
            if let Some(&price) = prices.get(date) {
                return Some(price);
            }
            // Carry forward: find most recent price <= date
            if let Some((&_, &price)) = prices.range(..=date.to_string()).next_back() {
                return Some(price);
            }
        }

        // Fall back to last trade price
        self.fallback.get(symbol).copied()
    }
}

/// Running position state for a single symbol
#[derive(Debug, Clone, Default)]
struct SymbolPosition {
    quantity: f64,
    total_cost_cents: i64,
}

/// Running state of all positions
struct PositionState {
    positions: HashMap<String, SymbolPosition>,
}

impl PositionState {
    fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    fn apply_activity(
        &mut self,
        symbol: &str,
        activity_type_str: &str,
        quantity: Option<f64>,
        unit_price_cents: Option<i64>,
        _fee_cents: i64,
    ) {
        let activity_type: TradingActivityType = activity_type_str
            .parse()
            .unwrap_or(TradingActivityType::Buy);
        let qty = quantity.unwrap_or(0.0);
        let price = unit_price_cents.unwrap_or(0);

        // Guard against invalid float values
        if !qty.is_finite() {
            return;
        }

        let entry = self.positions.entry(symbol.to_string()).or_default();

        match activity_type {
            TradingActivityType::Buy | TradingActivityType::AddHolding => {
                let cost = (qty * price as f64).round() as i64;
                entry.quantity += qty;
                entry.total_cost_cents = entry.total_cost_cents.saturating_add(cost);
            }
            TradingActivityType::Sell | TradingActivityType::RemoveHolding => {
                if entry.quantity > 0.0 {
                    let avg_cost = entry.total_cost_cents as f64 / entry.quantity;
                    let cost_reduction = (qty * avg_cost).round() as i64;
                    entry.quantity -= qty;
                    entry.total_cost_cents = entry.total_cost_cents.saturating_sub(cost_reduction);
                    if entry.quantity < 0.0 {
                        entry.quantity = 0.0;
                    }
                }
            }
            TradingActivityType::TransferIn => {
                let cost = (qty * price as f64).round() as i64;
                entry.quantity += qty;
                entry.total_cost_cents = entry.total_cost_cents.saturating_add(cost);
            }
            TradingActivityType::TransferOut => {
                if entry.quantity > 0.0 {
                    let avg_cost = entry.total_cost_cents as f64 / entry.quantity;
                    let cost_reduction = (qty * avg_cost).round() as i64;
                    entry.quantity -= qty;
                    entry.total_cost_cents = entry.total_cost_cents.saturating_sub(cost_reduction);
                    if entry.quantity < 0.0 {
                        entry.quantity = 0.0;
                    }
                }
            }
            TradingActivityType::Split => {
                // Split adjusts quantity but not total cost
                if qty > 0.0 {
                    entry.quantity *= qty;
                }
            }
            TradingActivityType::Deposit => {
                // Cash deposit
                let amount = (qty * price as f64).round() as i64;
                entry.quantity += qty;
                entry.total_cost_cents = entry.total_cost_cents.saturating_add(amount);
            }
            TradingActivityType::Withdrawal
            | TradingActivityType::Fee
            | TradingActivityType::Tax => {
                // These reduce cash
                let amount = (qty * price as f64).round() as i64;
                entry.quantity -= qty;
                entry.total_cost_cents = entry.total_cost_cents.saturating_sub(amount);
            }
            TradingActivityType::Dividend | TradingActivityType::Interest => {
                // These add cash
                let amount = (qty * price as f64).round() as i64;
                entry.quantity += qty;
                entry.total_cost_cents = entry.total_cost_cents.saturating_add(amount);
            }
        }
    }

    /// Calculate total portfolio value at given prices
    fn value_at_prices(&self, price_lookup: &PriceLookup, date: &str) -> i64 {
        let mut total = 0i64;

        for (symbol, pos) in &self.positions {
            if pos.quantity == 0.0 || !pos.quantity.is_finite() {
                continue;
            }

            if symbol.starts_with("$CASH-") {
                // Cash positions: value = cost basis (it's actual cash)
                total = total.saturating_add(pos.total_cost_cents);
            } else {
                // Security positions: value = qty * price
                let value = if let Some(price) = price_lookup.get_price(symbol, date) {
                    let calculated = pos.quantity * price as f64;
                    if calculated.is_finite() {
                        calculated.round() as i64
                    } else {
                        pos.total_cost_cents
                    }
                } else {
                    // Last resort: use cost basis
                    pos.total_cost_cents
                };
                total = total.saturating_add(value);
            }
        }

        total
    }
}

/// Generate all dates in range (inclusive)
fn generate_date_range(start: &str, end: &str) -> Vec<String> {
    let start_date = match NaiveDate::parse_from_str(start, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let end_date = match NaiveDate::parse_from_str(end, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    let mut dates = Vec::new();
    let mut current = start_date;
    while current <= end_date {
        dates.push(current.format("%Y-%m-%d").to_string());
        current += Duration::days(1);
    }
    dates
}

/// Build cumulative expense sums indexed by date
fn build_cumulative_expenses(daily_sums: &[(String, i64)]) -> BTreeMap<String, i64> {
    let mut cumulative = BTreeMap::new();
    let mut running_total = 0i64;

    for (date, amount) in daily_sums {
        running_total = running_total.saturating_add(*amount);
        cumulative.insert(date.clone(), running_total);
    }

    cumulative
}

/// Get cumulative expense value at date (carry forward if no exact match)
fn get_cumulative_at_date(cumulative: &BTreeMap<String, i64>, date: &str) -> i64 {
    // Try exact match first
    if let Some(&value) = cumulative.get(date) {
        return value;
    }
    // Carry forward: find most recent value <= date
    if let Some((&_, &value)) = cumulative.range(..=date.to_string()).next_back() {
        return value;
    }
    0
}

/// Calculate net worth history
pub fn calculate_net_worth_history(conn: &Connection) -> rusqlite::Result<NetWorthSummary> {
    // Get date range
    let start_date = match get_earliest_date(conn)? {
        Some(d) => d,
        None => return Ok(NetWorthSummary::empty()),
    };
    let end_date = match get_latest_date(conn)? {
        Some(d) => d,
        None => return Ok(NetWorthSummary::empty()),
    };

    // Pre-fetch all data
    let daily_expense_sums = get_daily_expense_sums(conn)?;
    let activities = get_all_activities_ordered(conn)?;
    let market_data = get_all_market_data(conn)?;
    let last_trade_prices = get_last_trade_prices(conn)?;

    // Build price lookup
    let mut price_lookup = PriceLookup::new();
    for (symbol, date, price) in &market_data {
        price_lookup.add_market_data(symbol, date, *price);
    }
    for (symbol, price, _date) in &last_trade_prices {
        price_lookup.set_fallback(symbol, *price);
    }

    // Build cumulative expense sums
    let cumulative_expenses = build_cumulative_expenses(&daily_expense_sums);

    // Generate date range
    let dates = generate_date_range(&start_date, &end_date);
    if dates.is_empty() {
        return Ok(NetWorthSummary::empty());
    }

    // Sweep through dates
    let mut position_state = PositionState::new();
    let mut activity_idx = 0;
    let mut data_points = Vec::with_capacity(dates.len());

    for date in &dates {
        // Apply all activities up to and including this date
        while activity_idx < activities.len() && activities[activity_idx].0 <= *date {
            let (_, symbol, activity_type, quantity, unit_price_cents, fee_cents, _currency) =
                &activities[activity_idx];
            position_state.apply_activity(
                symbol,
                activity_type,
                *quantity,
                *unit_price_cents,
                *fee_cents,
            );
            activity_idx += 1;
        }

        // Get cumulative expense value
        let expense_component = get_cumulative_at_date(&cumulative_expenses, date);

        // Calculate portfolio value
        let portfolio_component = position_state.value_at_prices(&price_lookup, date);

        // Net worth = expense cumulative + portfolio value
        let net_worth = expense_component.saturating_add(portfolio_component);

        data_points.push(NetWorthDataPoint {
            date: date.clone(),
            net_worth_cents: net_worth,
            expense_component_cents: expense_component,
            portfolio_component_cents: portfolio_component,
        });
    }

    Ok(NetWorthSummary::from_data_points(data_points))
}

/// Decimate data points for chart display (reduce to max_points)
/// Preserves first, last, min, and max points to ensure visual accuracy
pub fn decimate_for_display(
    data: &[NetWorthDataPoint],
    max_points: usize,
) -> Vec<NetWorthDataPoint> {
    if data.len() <= max_points || max_points == 0 {
        return data.to_vec();
    }

    // Find indices of min and max points
    let (min_idx, max_idx) = data
        .iter()
        .enumerate()
        .fold((0, 0), |(min_i, max_i), (i, p)| {
            let new_min = if p.net_worth_cents < data[min_i].net_worth_cents {
                i
            } else {
                min_i
            };
            let new_max = if p.net_worth_cents > data[max_i].net_worth_cents {
                i
            } else {
                max_i
            };
            (new_min, new_max)
        });

    let step = data.len() / max_points;
    let mut result: Vec<NetWorthDataPoint> = Vec::with_capacity(max_points + 4);

    for (i, point) in data.iter().enumerate() {
        // Include if it's on the step, or if it's a critical point (first, last, min, max)
        let is_step = i % step == 0;
        let is_first = i == 0;
        let is_last = i == data.len() - 1;
        let is_min = i == min_idx;
        let is_max = i == max_idx;

        if is_step || is_first || is_last || is_min || is_max {
            result.push(point.clone());
        }
    }

    // Sort by date to maintain chronological order (min/max might be out of order)
    result.sort_by(|a, b| a.date.cmp(&b.date));

    // Deduplicate (in case min/max landed on a step)
    result.dedup_by(|a, b| a.date == b.date);

    result
}
