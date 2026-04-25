use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub is_main: bool,
    pub birthday: Option<String>,
    pub desired_retirement_age: Option<i64>,
    pub marriage_status: String,
    pub current_portfolio_override_cents: Option<i64>,
    pub monthly_savings_cents: Option<i64>,
    pub assumed_roi: f64,
    pub expected_inflation: f64,
    pub monthly_living_costs_cents: Option<i64>,
    pub tax_rate: f64,
    pub monthly_pension_cents: Option<i64>,
    pub official_retirement_age: Option<i64>,
    pub life_expectancy: i64,
    /// Total amount deposited into investments (cost basis). Used for FIFO tax calculation.
    /// If None, defaults to DB-computed total from trading history.
    pub deposits_cents: Option<i64>,
    /// Barista FIRE: part-time income between early retirement and official pension age.
    /// Applied in today's money; inflated in the simulation. Reduces portfolio withdrawals.
    pub monthly_barista_income_cents: Option<i64>,
    /// Annual rate at which monthly savings grow (e.g. 0.02 = 2% raises per year).
    pub savings_growth_rate: f64,
    pub created_at: String,
    pub updated_at: String,
}

impl Scenario {
    pub fn monthly_savings_display(&self) -> String {
        self.monthly_savings_cents
            .map(|c| format!("{:.2}", c as f64 / 100.0))
            .unwrap_or_default()
    }

    pub fn monthly_living_costs_display(&self) -> String {
        self.monthly_living_costs_cents
            .map(|c| format!("{:.2}", c as f64 / 100.0))
            .unwrap_or_default()
    }

    pub fn monthly_pension_display(&self) -> String {
        self.monthly_pension_cents
            .map(|c| format!("{:.2}", c as f64 / 100.0))
            .unwrap_or_default()
    }

    pub fn portfolio_override_display(&self) -> String {
        self.current_portfolio_override_cents
            .map(|c| format!("{:.2}", c as f64 / 100.0))
            .unwrap_or_default()
    }

    pub fn roi_display(&self) -> String {
        format!("{:.2}", self.assumed_roi * 100.0)
    }

    pub fn inflation_display(&self) -> String {
        format!("{:.2}", self.expected_inflation * 100.0)
    }

    pub fn tax_rate_display(&self) -> String {
        format!("{:.3}", self.tax_rate * 100.0)
    }

    pub fn deposits_display(&self) -> String {
        self.deposits_cents
            .map(|c| format!("{:.2}", c as f64 / 100.0))
            .unwrap_or_default()
    }

    pub fn monthly_barista_income_display(&self) -> String {
        self.monthly_barista_income_cents
            .map(|c| format!("{:.2}", c as f64 / 100.0))
            .unwrap_or_default()
    }

    pub fn savings_growth_display(&self) -> String {
        format!("{:.2}", self.savings_growth_rate * 100.0)
    }
}

/// In-memory result of a Monte Carlo simulation run — not persisted to DB.
#[derive(Debug, Clone)]
pub struct MonteCarloResult {
    pub success_probability: f64,
    pub early_retirement_p10_age: Option<f64>,
    pub early_retirement_p50_age: Option<f64>,
    pub early_retirement_p90_age: Option<f64>,
    pub years: Vec<i32>,
    pub p10: Vec<i64>,
    pub p25: Vec<i64>,
    pub p50: Vec<i64>,
    pub p75: Vec<i64>,
    pub p90: Vec<i64>,
}

/// Chart data served as JSON to the TypeScript ECharts client.
#[derive(Debug, Clone, Serialize)]
pub struct RetirementChartData {
    pub years: Vec<i32>,
    /// Portfolio value in today's (real) cents
    pub p10: Vec<i64>,
    pub p25: Vec<i64>,
    pub p50: Vec<i64>,
    pub p75: Vec<i64>,
    pub p90: Vec<i64>,
    pub deterministic: Vec<i64>,
    pub retirement_year: Option<i32>,
    pub pension_year: Option<i32>,
    pub life_expectancy_year: Option<i32>,
}

/// JSON response for POST /api/retirement/simulate (used by the slider UI).
#[derive(Debug, Serialize)]
pub struct SimulateResponse {
    pub success_probability_display: Option<String>,
    pub success_color_class: &'static str,
    pub early_retirement_p10_display: Option<String>,
    pub early_retirement_p50_display: Option<String>,
    pub early_retirement_p90_display: Option<String>,
    pub chart: RetirementChartData,
}

/// One row in the savings-phase table.
#[derive(Debug, Clone)]
pub struct SavingsRow {
    pub year: i32,
    pub age_display: String,
    pub portfolio_start_cents: i64,
    pub annual_savings_cents: i64,
    /// Nominal investment gain for the year (portfolio × ROI, no tax during accumulation).
    pub gain_cents: i64,
    pub portfolio_end_cents: i64,
}

/// One row in the withdrawal-phase table.
#[derive(Debug, Clone)]
pub struct WithdrawalRow {
    pub year: i32,
    pub age_display: String,
    pub portfolio_start_cents: i64,
    pub withdrawal_cents: i64,
    pub pension_cents: i64,
    /// Barista FIRE income for this year (0 once pension kicks in).
    pub barista_cents: i64,
    pub net_withdrawal_cents: i64,
    /// Unrealized investment gain for the year (portfolio × ROI, not taxed until sold).
    pub gain_cents: i64,
    /// Tax paid on the realized-gain portion of the withdrawal (FIFO).
    pub tax_cents: i64,
    pub portfolio_end_cents: i64,
    pub is_ruined: bool,
}

/// Full computed data bundle for the page template.
#[derive(Debug, Clone)]
pub struct RetirementProjection {
    pub scenario: Scenario,
    /// True when inputs are complete and the simulation ran successfully.
    pub has_simulation: bool,
    pub savings_rows: Vec<SavingsRow>,
    pub withdrawal_rows: Vec<WithdrawalRow>,
    pub success_probability_display: Option<String>,
    /// Tailwind text color class based on success probability (red/yellow/green).
    pub success_color_class: &'static str,
    pub early_retirement_p10_display: Option<String>,
    pub early_retirement_p50_display: Option<String>,
    pub early_retirement_p90_display: Option<String>,
    pub current_portfolio_cents: i64,
}
