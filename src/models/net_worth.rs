use serde::Serialize;

/// A single data point in the net worth time series
#[derive(Debug, Clone, Serialize)]
pub struct NetWorthDataPoint {
    pub date: String,
    pub net_worth_cents: i64,
    pub expense_component_cents: i64,
    pub portfolio_component_cents: i64,
}

/// Summary of net worth calculation results
#[derive(Debug, Clone, Serialize)]
pub struct NetWorthSummary {
    pub data_points: Vec<NetWorthDataPoint>,
    pub current_net_worth_cents: i64,
    pub highest_net_worth_cents: i64,
    pub lowest_net_worth_cents: i64,
    pub start_date: String,
    pub end_date: String,
}

impl NetWorthSummary {
    pub fn empty() -> Self {
        Self {
            data_points: Vec::new(),
            current_net_worth_cents: 0,
            highest_net_worth_cents: 0,
            lowest_net_worth_cents: 0,
            start_date: String::new(),
            end_date: String::new(),
        }
    }

    pub fn from_data_points(data_points: Vec<NetWorthDataPoint>) -> Self {
        if data_points.is_empty() {
            return Self::empty();
        }

        let current = data_points.last().map(|p| p.net_worth_cents).unwrap_or(0);
        let highest = data_points
            .iter()
            .map(|p| p.net_worth_cents)
            .max()
            .unwrap_or(0);
        let lowest = data_points
            .iter()
            .map(|p| p.net_worth_cents)
            .min()
            .unwrap_or(0);
        let start_date = data_points
            .first()
            .map(|p| p.date.clone())
            .unwrap_or_default();
        let end_date = data_points
            .last()
            .map(|p| p.date.clone())
            .unwrap_or_default();

        Self {
            data_points,
            current_net_worth_cents: current,
            highest_net_worth_cents: highest,
            lowest_net_worth_cents: lowest,
            start_date,
            end_date,
        }
    }
}
