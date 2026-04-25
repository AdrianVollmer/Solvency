use rand_distr::{Distribution, LogNormal};

use crate::error::AppResult;
use crate::models::retirement::{
    MonteCarloResult, RetirementChartData, Scenario, SavingsRow, WithdrawalRow,
};

/// Annual return volatility assumed for all simulations (15% std dev).
const SIGMA: f64 = 0.15;
/// Number of Monte Carlo iterations.
const N_SIMULATIONS: usize = 1000;

/// All inputs needed for a projection, pre-computed from a `Scenario`.
#[derive(Debug, Clone)]
pub struct ProjectionInputs {
    pub current_age: f64,
    pub retirement_age: f64,
    pub life_expectancy: f64,
    pub official_pension_age: f64,
    pub current_portfolio_cents: i64,
    /// Known cost basis (total historical deposits) at the start of the projection.
    pub cost_basis_cents: i64,
    pub annual_savings_cents: i64,
    pub roi: f64,
    pub inflation: f64,
    pub tax_rate: f64,
    pub annual_living_costs_cents: i64,
    pub annual_pension_cents: i64,
    /// Barista FIRE income per year in today's money. Applied from retirement until pension age.
    pub annual_barista_income_cents: i64,
    /// Annual rate at which savings grow (e.g. 0.02 = 2% per year due to raises).
    pub savings_growth_rate: f64,
}

impl ProjectionInputs {
    pub fn from_scenario(
        scenario: &Scenario,
        current_portfolio_cents: i64,
        cost_basis_cents: i64,
        today_year: i32,
    ) -> Option<Self> {
        let birthday = scenario.birthday.as_deref()?;
        let desired_retirement_age = scenario.desired_retirement_age? as f64;
        let birth_year: i32 = birthday.split('-').next()?.parse().ok()?;

        let current_age = (today_year - birth_year) as f64;

        Some(Self {
            current_age,
            retirement_age: desired_retirement_age,
            life_expectancy: scenario.life_expectancy as f64,
            official_pension_age: scenario.official_retirement_age.unwrap_or(67) as f64,
            current_portfolio_cents,
            cost_basis_cents,
            annual_savings_cents: scenario.monthly_savings_cents.unwrap_or(0) * 12,
            roi: scenario.assumed_roi,
            inflation: scenario.expected_inflation,
            tax_rate: scenario.tax_rate,
            annual_living_costs_cents: scenario.monthly_living_costs_cents.unwrap_or(0) * 12,
            annual_pension_cents: scenario.monthly_pension_cents.unwrap_or(0) * 12,
            annual_barista_income_cents: scenario.monthly_barista_income_cents.unwrap_or(0) * 12,
            savings_growth_rate: scenario.savings_growth_rate,
        })
    }
}

/// Compute the deterministic savings phase, year by year.
///
/// Tax is not applied during accumulation — capital gains are largely unrealized
/// until sold, so we model a simple gross-return compounding.
pub fn savings_phase(inputs: &ProjectionInputs, today_year: i32) -> Vec<SavingsRow> {
    let years_to_retirement = ((inputs.retirement_age - inputs.current_age).ceil() as usize).max(0);
    let mut rows = Vec::with_capacity(years_to_retirement);
    let mut portfolio = inputs.current_portfolio_cents as f64;

    for y in 0..years_to_retirement {
        let portfolio_start = portfolio as i64;
        let savings = inputs.annual_savings_cents as f64
            * (1.0 + inputs.savings_growth_rate).powi(y as i32);
        let gain = portfolio * inputs.roi;
        let portfolio_end = portfolio + gain + savings;

        rows.push(SavingsRow {
            year: today_year + y as i32,
            age_display: format!("{:.0}", inputs.current_age + y as f64),
            portfolio_start_cents: portfolio_start,
            annual_savings_cents: savings.round() as i64,
            gain_cents: gain.round() as i64,
            portfolio_end_cents: portfolio_end.round() as i64,
        });

        portfolio = portfolio_end;
    }

    rows
}

/// Compute the deterministic withdrawal phase, year by year.
///
/// Tax model: FIFO average cost basis.  Investment gains accrue unrealised (no annual tax).
/// When selling shares to fund a withdrawal, only the *gain fraction* of the proceeds is taxed.
///   gain_fraction  = max(0, 1 – cost_basis / portfolio)
///   gross_withdrawal = net_needed / (1 – gain_fraction × tax_rate)
///   tax = gross_withdrawal – net_needed
/// Cost basis decreases by the deposit portion of each gross withdrawal.
pub fn withdrawal_phase(
    inputs: &ProjectionInputs,
    portfolio_at_retirement_cents: i64,
    cost_basis_at_retirement_cents: i64,
    retirement_year: i32,
) -> Vec<WithdrawalRow> {
    let years = ((inputs.life_expectancy - inputs.retirement_age).ceil() as usize).max(0);
    let mut rows = Vec::with_capacity(years);
    let mut portfolio = portfolio_at_retirement_cents as f64;
    let mut cost_basis = cost_basis_at_retirement_cents as f64;

    for y in 0..years {
        let is_ruined = portfolio <= 0.0;
        let portfolio_start = portfolio.max(0.0) as i64;

        let years_from_today = inputs.retirement_age - inputs.current_age + y as f64;
        let inflation_factor = (1.0 + inputs.inflation).powf(years_from_today);
        let age_this_year = inputs.retirement_age + y as f64;

        let withdrawal = inputs.annual_living_costs_cents as f64 * inflation_factor;
        let pension = if age_this_year >= inputs.official_pension_age {
            inputs.annual_pension_cents as f64 * inflation_factor
        } else {
            0.0
        };
        // Barista income applies from retirement until pension starts.
        let barista = if age_this_year < inputs.official_pension_age {
            inputs.annual_barista_income_cents as f64 * inflation_factor
        } else {
            0.0
        };
        let net_needed = (withdrawal - pension - barista).max(0.0);

        // FIFO gain fraction: proportion of portfolio that is unrealised gains.
        let gain_fraction = if portfolio > 0.0 {
            ((portfolio - cost_basis) / portfolio).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let effective_tax_rate = gain_fraction * inputs.tax_rate;

        let (gross_withdrawal, tax) = if net_needed > 0.0 && !is_ruined {
            let gross = net_needed / (1.0 - effective_tax_rate);
            (gross, gross - net_needed)
        } else {
            (0.0, 0.0)
        };

        // Investment gain is unrealised — portfolio grows but no tax until sold.
        let gain = portfolio.max(0.0) * inputs.roi;
        let portfolio_end = if is_ruined {
            0.0
        } else {
            (portfolio + gain - gross_withdrawal).max(0.0)
        };

        // Reduce cost basis by the deposit portion of what was sold.
        let deposit_fraction = 1.0 - gain_fraction;
        cost_basis = (cost_basis - gross_withdrawal * deposit_fraction).max(0.0);
        portfolio = portfolio_end;

        rows.push(WithdrawalRow {
            year: retirement_year + y as i32,
            age_display: format!("{:.0}", age_this_year),
            portfolio_start_cents: portfolio_start,
            withdrawal_cents: withdrawal.round() as i64,
            pension_cents: pension.round() as i64,
            barista_cents: barista.round() as i64,
            net_withdrawal_cents: net_needed.round() as i64,
            gain_cents: gain.round() as i64,
            tax_cents: tax.round() as i64,
            portfolio_end_cents: portfolio_end.round() as i64,
            is_ruined,
        });
    }

    rows
}

/// Simulate a single run for `total_years` years, returning real portfolio value per year.
///
/// `LogNormal::new(mu, sigma)` samples a growth *factor* (e.g. ~1.07 for 7% mean return).
/// We subtract 1 to get the gain fraction.  FIFO cost-basis tracking mirrors `withdrawal_phase`.
fn simulate_once(inputs: &ProjectionInputs, rng: &mut impl rand::Rng) -> Vec<f64> {
    let total_years = (inputs.life_expectancy - inputs.current_age).ceil() as usize;
    let years_savings = (inputs.retirement_age - inputs.current_age).ceil() as usize;

    // mu chosen so that E[factor] = e^(mu + σ²/2) = e^roi
    let mu = inputs.roi - 0.5 * SIGMA * SIGMA;
    let log_normal = LogNormal::new(mu, SIGMA).expect("valid lognormal params");

    let mut portfolio = inputs.current_portfolio_cents as f64;
    let mut cost_basis = inputs.cost_basis_cents as f64;
    let mut real_vals = Vec::with_capacity(total_years);

    for y in 0..total_years {
        let return_factor = log_normal.sample(rng); // e.g. ~1.07
        let gain = portfolio * (return_factor - 1.0);

        if y < years_savings {
            // Accumulation: savings grow each year by the configured rate.
            let savings = inputs.annual_savings_cents as f64
                * (1.0 + inputs.savings_growth_rate).powi(y as i32);
            portfolio = (portfolio + gain + savings).max(0.0);
            cost_basis += savings;
        } else {
            let withdrawal_y = y - years_savings;
            let years_from_today =
                (inputs.retirement_age - inputs.current_age) + withdrawal_y as f64;
            let inflation_factor = (1.0 + inputs.inflation).powf(years_from_today);

            let withdrawal = inputs.annual_living_costs_cents as f64 * inflation_factor;
            let age_this_year = inputs.retirement_age + withdrawal_y as f64;
            let pension = if age_this_year >= inputs.official_pension_age {
                inputs.annual_pension_cents as f64 * inflation_factor
            } else {
                0.0
            };
            let barista = if age_this_year < inputs.official_pension_age {
                inputs.annual_barista_income_cents as f64 * inflation_factor
            } else {
                0.0
            };
            let net_needed = (withdrawal - pension - barista).max(0.0);

            let gain_fraction = if portfolio > 0.0 {
                ((portfolio - cost_basis) / portfolio).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let gross_withdrawal = if net_needed > 0.0 {
                net_needed / (1.0 - gain_fraction * inputs.tax_rate)
            } else {
                0.0
            };

            // Unrealised gain grows portfolio; reduce cost basis by deposit portion sold.
            let deposit_fraction = 1.0 - gain_fraction;
            cost_basis = (cost_basis - gross_withdrawal * deposit_fraction).max(0.0);
            portfolio = (portfolio + gain - gross_withdrawal).max(0.0);
        }

        let inf = (1.0 + inputs.inflation).powf(y as f64);
        real_vals.push(if inf > 0.0 { portfolio / inf } else { 0.0 });
    }

    real_vals
}

/// Percentile of a sorted slice using linear interpolation.
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p / 100.0) * (sorted.len() - 1) as f64;
    let lo = idx.floor() as usize;
    let hi = (lo + 1).min(sorted.len() - 1);
    let frac = idx - lo as f64;
    sorted[lo] * (1.0 - frac) + sorted[hi] * frac
}

/// Run full Monte Carlo and return an in-memory `MonteCarloResult`.
pub fn run_monte_carlo(
    inputs: &ProjectionInputs,
    today_year: i32,
) -> AppResult<MonteCarloResult> {
    let total_years = (inputs.life_expectancy - inputs.current_age).ceil() as usize;

    let mut rng = rand::thread_rng();
    let mut all_runs: Vec<Vec<f64>> = Vec::with_capacity(N_SIMULATIONS);
    let mut success_count = 0usize;

    for _ in 0..N_SIMULATIONS {
        let trajectory = simulate_once(inputs, &mut rng);
        if trajectory.last().copied().unwrap_or(0.0) > 0.0 {
            success_count += 1;
        }
        all_runs.push(trajectory);
    }

    let success_probability = success_count as f64 / N_SIMULATIONS as f64;

    // Build percentile bands per year
    let mut p10 = Vec::with_capacity(total_years);
    let mut p25 = Vec::with_capacity(total_years);
    let mut p50 = Vec::with_capacity(total_years);
    let mut p75 = Vec::with_capacity(total_years);
    let mut p90 = Vec::with_capacity(total_years);

    for y in 0..total_years {
        let mut vals: Vec<f64> = all_runs.iter().map(|r| r[y]).collect();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        p10.push(percentile_sorted(&vals, 10.0) as i64);
        p25.push(percentile_sorted(&vals, 25.0) as i64);
        p50.push(percentile_sorted(&vals, 50.0) as i64);
        p75.push(percentile_sorted(&vals, 75.0) as i64);
        p90.push(percentile_sorted(&vals, 90.0) as i64);
    }

    // Determine the earliest age at which each confidence level is achievable.
    // Search from (current_age + 1) up to (life_expectancy - 5) so results are shown even
    // when the desired retirement age itself doesn't meet the threshold.
    let min_age = (inputs.current_age + 1.0) as i64;
    let max_age = (inputs.life_expectancy - 5.0).max(inputs.current_age + 2.0) as i64;

    let find_viable_age = |target_rate: f64| -> Option<f64> {
        let mut batch_rng = rand::thread_rng();
        for candidate_age in min_age..=max_age {
            let modified = ProjectionInputs {
                retirement_age: candidate_age as f64,
                ..inputs.clone()
            };
            let n = 200usize;
            let successes: usize = (0..n)
                .map(|_| {
                    if simulate_once(&modified, &mut batch_rng)
                        .last()
                        .copied()
                        .unwrap_or(0.0)
                        > 0.0
                    {
                        1
                    } else {
                        0
                    }
                })
                .sum();
            if successes as f64 / n as f64 >= target_rate {
                return Some(candidate_age as f64);
            }
        }
        None
    };

    let early_retirement_p90_age = find_viable_age(0.90);
    let early_retirement_p50_age = find_viable_age(0.50);
    let early_retirement_p10_age = find_viable_age(0.10);

    let years: Vec<i32> = (0..total_years)
        .map(|y| today_year + y as i32)
        .collect();

    Ok(MonteCarloResult {
        success_probability,
        early_retirement_p10_age,
        early_retirement_p50_age,
        early_retirement_p90_age,
        years,
        p10,
        p25,
        p50,
        p75,
        p90,
    })
}

/// Build `RetirementChartData` from a `MonteCarloResult` + deterministic projection.
pub fn build_chart_data(
    mc: &MonteCarloResult,
    inputs: &ProjectionInputs,
    today_year: i32,
) -> AppResult<RetirementChartData> {
    let savings = savings_phase(inputs, today_year);
    let portfolio_at_retirement = savings
        .last()
        .map(|r| r.portfolio_end_cents)
        .unwrap_or(inputs.current_portfolio_cents);
    let years_savings = savings.len() as i64;
    let cost_basis_at_retirement =
        inputs.cost_basis_cents + inputs.annual_savings_cents * years_savings;
    let retirement_year =
        today_year + (inputs.retirement_age - inputs.current_age).ceil() as i32;
    let withdrawal =
        withdrawal_phase(inputs, portfolio_at_retirement, cost_basis_at_retirement, retirement_year);

    let mut deterministic: Vec<i64> = savings
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let inf = (1.0 + inputs.inflation).powi(i as i32 + 1);
            (r.portfolio_end_cents as f64 / inf) as i64
        })
        .collect();
    // Withdrawal rows: real value = portfolio_end / inflation_factor
    let years_to_retirement = savings.len() as f64;
    deterministic.extend(withdrawal.iter().enumerate().map(|(i, r)| {
        let yrs = years_to_retirement + i as f64;
        let inf = (1.0 + inputs.inflation).powf(yrs);
        (r.portfolio_end_cents as f64 / inf) as i64
    }));

    let retirement_year_val =
        Some(today_year + (inputs.retirement_age - inputs.current_age).ceil() as i32);
    let pension_year_val =
        Some(today_year + (inputs.official_pension_age - inputs.current_age).ceil() as i32);
    let life_expectancy_year_val =
        Some(today_year + (inputs.life_expectancy - inputs.current_age).ceil() as i32);

    Ok(RetirementChartData {
        years: mc.years.clone(),
        p10: mc.p10.clone(),
        p25: mc.p25.clone(),
        p50: mc.p50.clone(),
        p75: mc.p75.clone(),
        p90: mc.p90.clone(),
        deterministic,
        retirement_year: retirement_year_val,
        pension_year: pension_year_val,
        life_expectancy_year: life_expectancy_year_val,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_inputs() -> ProjectionInputs {
        ProjectionInputs {
            current_age: 40.0,
            retirement_age: 65.0,
            life_expectancy: 90.0,
            official_pension_age: 67.0,
            current_portfolio_cents: 100_000_00, // 100 000 €
            cost_basis_cents: 50_000_00,         // 50 000 € deposits
            annual_savings_cents: 24_000_00,     // 2 000 €/month
            roi: 0.07,
            inflation: 0.02,
            tax_rate: 0.26375,
            annual_living_costs_cents: 36_000_00, // 3 000 €/month
            annual_pension_cents: 0,
            annual_barista_income_cents: 0,
            savings_growth_rate: 0.0,
        }
    }

    // ── savings_phase ────────────────────────────────────────────────────────

    #[test]
    fn savings_phase_length_matches_years_to_retirement() {
        let inputs = base_inputs();
        let rows = savings_phase(&inputs, 2025);
        assert_eq!(rows.len(), 25); // ceil(65 - 40) = 25
    }

    #[test]
    fn savings_phase_portfolio_grows_monotonically() {
        let inputs = base_inputs();
        let rows = savings_phase(&inputs, 2025);
        for i in 1..rows.len() {
            assert!(
                rows[i].portfolio_end_cents > rows[i - 1].portfolio_end_cents,
                "Portfolio should grow each year (year {}): {} <= {}",
                i,
                rows[i].portfolio_end_cents,
                rows[i - 1].portfolio_end_cents
            );
        }
    }

    #[test]
    fn savings_phase_year_one_matches_manual_calculation() {
        // portfolio_end = 100 000 + 100 000 * 7% + 24 000 = 131 000 €
        let inputs = base_inputs();
        let rows = savings_phase(&inputs, 2025);
        let row = &rows[0];
        assert_eq!(row.gain_cents, 7_000_00); // 7 000 €
        assert_eq!(row.annual_savings_cents, 24_000_00);
        assert_eq!(row.portfolio_end_cents, 131_000_00); // 131 000 €
    }

    #[test]
    fn savings_phase_no_portfolio_gives_only_savings_compounding() {
        let mut inputs = base_inputs();
        inputs.current_portfolio_cents = 0;
        inputs.current_age = 64.0;
        inputs.retirement_age = 65.0;
        let rows = savings_phase(&inputs, 2025);
        assert_eq!(rows.len(), 1);
        // gain = 0 * 7% = 0; portfolio_end = 0 + 24 000 = 24 000
        assert_eq!(rows[0].gain_cents, 0);
        assert_eq!(rows[0].portfolio_end_cents, 24_000_00);
    }

    #[test]
    fn savings_phase_empty_when_already_at_retirement_age() {
        let mut inputs = base_inputs();
        inputs.current_age = 65.0;
        let rows = savings_phase(&inputs, 2025);
        assert!(rows.is_empty());
    }

    // ── withdrawal_phase ─────────────────────────────────────────────────────

    #[test]
    fn withdrawal_phase_depletes_tiny_portfolio() {
        // 1 000 € portfolio, 36 000 € annual costs → ruined in year 1
        let mut inputs = base_inputs();
        inputs.annual_pension_cents = 0;
        let rows = withdrawal_phase(&inputs, 1_000_00, 0, 2065);
        assert!(!rows.is_empty());
        // First row: is_ruined because portfolio is positive but after one
        // withdrawal it goes to zero.
        let first_ruined = rows.iter().position(|r| r.is_ruined).unwrap_or(rows.len());
        assert!(first_ruined <= 2, "tiny portfolio should deplete within 2 years");
    }

    #[test]
    fn withdrawal_phase_pension_reduces_net_withdrawal() {
        let mut inputs = base_inputs();
        // Set pension to cover full living costs from day 1 of retirement
        inputs.annual_pension_cents = 36_000_00;
        inputs.official_pension_age = 65.0; // same as retirement age
        // Large enough portfolio that investment returns easily cover anything
        let rows = withdrawal_phase(&inputs, 1_000_000_00, 0, 2065);
        // Net withdrawal should be 0 (pension covers costs), so portfolio should grow
        for row in &rows {
            assert_eq!(row.net_withdrawal_cents, 0, "pension should cover all costs");
            assert!(!row.is_ruined);
        }
    }

    #[test]
    fn withdrawal_phase_length_matches_years_in_retirement() {
        let inputs = base_inputs();
        let rows = withdrawal_phase(&inputs, 1_000_000_00, 0, 2065);
        assert_eq!(rows.len(), 25); // ceil(90 - 65) = 25
    }

    #[test]
    fn withdrawal_phase_is_ruined_propagates() {
        // Zero portfolio → all rows ruined immediately
        let rows = withdrawal_phase(&base_inputs(), 0, 0, 2065);
        assert!(rows.iter().all(|r| r.is_ruined));
    }

    // ── simulate_once (Monte Carlo) ──────────────────────────────────────────

    #[test]
    fn simulate_once_returns_correct_number_of_years() {
        let inputs = base_inputs();
        let mut rng = rand::thread_rng();
        let result = simulate_once(&inputs, &mut rng);
        assert_eq!(result.len(), 50); // ceil(90 - 40) = 50
    }

    #[test]
    fn simulate_once_portfolio_stays_in_plausible_range() {
        // Run 20 simulations; every output value should be in [0, 1e13]
        // (not 1e15+ which the old bug produced)
        let inputs = base_inputs();
        let mut rng = rand::thread_rng();
        for _ in 0..20 {
            let vals = simulate_once(&inputs, &mut rng);
            for (y, &v) in vals.iter().enumerate() {
                assert!(
                    v >= 0.0 && v < 1e13,
                    "year {y}: real portfolio {v} is out of range"
                );
            }
        }
    }

    #[test]
    fn simulate_once_doomed_scenario_usually_hits_zero() {
        // Starting portfolio = 0, zero savings, huge costs → almost always ruined
        let mut inputs = base_inputs();
        inputs.current_portfolio_cents = 0;
        inputs.annual_savings_cents = 0;
        inputs.annual_living_costs_cents = 1_000_000_00; // 1M/year costs
        let mut rng = rand::thread_rng();
        let mut ruined_count = 0usize;
        for _ in 0..50 {
            let vals = simulate_once(&inputs, &mut rng);
            if vals.last().copied().unwrap_or(1.0) == 0.0 {
                ruined_count += 1;
            }
        }
        assert!(
            ruined_count >= 45,
            "at least 90% of doomed runs should be ruined, got {ruined_count}/50"
        );
    }

    // ── run_monte_carlo ───────────────────────────────────────────────────────

    #[test]
    fn monte_carlo_success_probability_not_always_100_for_tight_scenario() {
        let mut inputs = base_inputs();
        inputs.current_portfolio_cents = 0;
        inputs.annual_living_costs_cents = 60_000_00;
        inputs.annual_savings_cents = 12_000_00;
        let result = run_monte_carlo(&inputs, 2025).unwrap();
        assert!(
            result.success_probability < 0.999,
            "tight scenario should not have 100% success, got {:.1}%",
            result.success_probability * 100.0
        );
        assert!(result.success_probability > 0.0, "scenario should succeed sometimes");
    }

    #[test]
    fn monte_carlo_doomed_scenario_has_low_success_probability() {
        let mut inputs = base_inputs();
        inputs.current_portfolio_cents = 0;
        inputs.annual_savings_cents = 0;
        inputs.annual_living_costs_cents = 500_000_00;
        let result = run_monte_carlo(&inputs, 2025).unwrap();
        assert!(
            result.success_probability < 0.05,
            "doomed scenario success should be <5%, got {:.1}%",
            result.success_probability * 100.0
        );
    }

    #[test]
    fn monte_carlo_wealthy_scenario_has_high_success_probability() {
        let mut inputs = base_inputs();
        inputs.current_portfolio_cents = 5_000_000_00;
        inputs.annual_savings_cents = 60_000_00;
        inputs.annual_living_costs_cents = 36_000_00;
        let result = run_monte_carlo(&inputs, 2025).unwrap();
        assert!(
            result.success_probability > 0.90,
            "wealthy scenario should succeed >90%, got {:.1}%",
            result.success_probability * 100.0
        );
    }

    #[test]
    fn monte_carlo_p10_age_lte_p50_lte_p90() {
        let result = run_monte_carlo(&base_inputs(), 2025).unwrap();
        if let (Some(p10), Some(p50), Some(p90)) = (
            result.early_retirement_p10_age,
            result.early_retirement_p50_age,
            result.early_retirement_p90_age,
        ) {
            assert!(p10 <= p50, "p10 age ({p10}) should be <= p50 age ({p50})");
            assert!(p50 <= p90, "p50 age ({p50}) should be <= p90 age ({p90})");
        }
    }

    #[test]
    fn monte_carlo_percentile_bands_have_correct_length() {
        let result = run_monte_carlo(&base_inputs(), 2025).unwrap();
        let total_years = 50usize; // ceil(90 - 40)
        assert_eq!(result.years.len(), total_years);
        assert_eq!(result.p10.len(), total_years);
        assert_eq!(result.p50.len(), total_years);
        assert_eq!(result.p90.len(), total_years);
    }

    #[test]
    fn monte_carlo_p10_band_lte_p90_band_per_year() {
        let result = run_monte_carlo(&base_inputs(), 2025).unwrap();
        for (y, (lo, hi)) in result.p10.iter().zip(result.p90.iter()).enumerate() {
            assert!(lo <= hi, "year {y}: p10 ({lo}) > p90 ({hi})");
        }
    }
}
