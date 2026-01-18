use chrono::NaiveDate;

/// A cash flow with a date and amount
#[derive(Debug, Clone)]
pub struct CashFlow {
    pub date: NaiveDate,
    pub amount: f64, // positive = inflow, negative = outflow
}

/// Calculate XIRR (Extended Internal Rate of Return) using Newton-Raphson method
///
/// XIRR finds the discount rate that makes the net present value of all cash flows equal to zero.
/// Cash flows should include:
/// - Negative amounts for investments (buys)
/// - Positive amounts for returns (sells, dividends)
/// - A final positive amount representing the current value of holdings
///
/// Returns None if:
/// - Less than 2 cash flows
/// - All cash flows are the same sign (no investment or no return)
/// - The algorithm doesn't converge
pub fn calculate_xirr(cash_flows: &[CashFlow]) -> Option<f64> {
    if cash_flows.len() < 2 {
        return None;
    }

    // Check that we have both positive and negative cash flows
    let has_negative = cash_flows.iter().any(|cf| cf.amount < 0.0);
    let has_positive = cash_flows.iter().any(|cf| cf.amount > 0.0);
    if !has_negative || !has_positive {
        return None;
    }

    // Find the earliest date to use as base
    let base_date = cash_flows.iter().map(|cf| cf.date).min()?;

    // Newton-Raphson iteration
    let mut rate = 0.1; // Initial guess of 10%
    let max_iterations = 100;
    let tolerance = 1e-7;

    for _ in 0..max_iterations {
        let (npv, npv_derivative) = calculate_npv_and_derivative(cash_flows, base_date, rate);

        if npv_derivative.abs() < 1e-10 {
            // Derivative too small, try a different starting point
            rate += 0.1;
            continue;
        }

        let new_rate = rate - npv / npv_derivative;

        // Check for convergence
        if (new_rate - rate).abs() < tolerance {
            // Validate the result is reasonable (between -99% and 10000%)
            if new_rate > -0.99 && new_rate < 100.0 {
                return Some(new_rate);
            }
            return None;
        }

        // Bound the rate to prevent divergence
        rate = new_rate.clamp(-0.99, 100.0);
    }

    // Try alternative starting points if initial guess didn't converge
    for initial_guess in [-0.5, 0.0, 0.5, 1.0, 2.0] {
        rate = initial_guess;
        for _ in 0..max_iterations {
            let (npv, npv_derivative) = calculate_npv_and_derivative(cash_flows, base_date, rate);

            if npv_derivative.abs() < 1e-10 {
                break;
            }

            let new_rate = rate - npv / npv_derivative;

            if (new_rate - rate).abs() < tolerance {
                if new_rate > -0.99 && new_rate < 100.0 {
                    return Some(new_rate);
                }
                break;
            }

            rate = new_rate.clamp(-0.99, 100.0);
        }
    }

    None
}

/// Calculate NPV and its derivative with respect to rate
fn calculate_npv_and_derivative(
    cash_flows: &[CashFlow],
    base_date: NaiveDate,
    rate: f64,
) -> (f64, f64) {
    let mut npv = 0.0;
    let mut npv_derivative = 0.0;

    for cf in cash_flows {
        let days = (cf.date - base_date).num_days() as f64;
        let years = days / 365.0;

        let discount_factor = (1.0 + rate).powf(-years);
        npv += cf.amount * discount_factor;

        // Derivative: d/dr [amount * (1+r)^(-t)] = -t * amount * (1+r)^(-t-1)
        npv_derivative -= years * cf.amount * (1.0 + rate).powf(-years - 1.0);
    }

    (npv, npv_derivative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_xirr() {
        // Invest $1000, receive $1100 one year later = 10% return
        let cash_flows = vec![
            CashFlow {
                date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
                amount: -1000.0,
            },
            CashFlow {
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                amount: 1100.0,
            },
        ];

        let xirr = calculate_xirr(&cash_flows).unwrap();
        assert!((xirr - 0.10).abs() < 0.001);
    }

    #[test]
    fn test_multiple_cash_flows() {
        // Multiple investments and returns
        let cash_flows = vec![
            CashFlow {
                date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
                amount: -1000.0,
            },
            CashFlow {
                date: NaiveDate::from_ymd_opt(2023, 6, 1).unwrap(),
                amount: -500.0,
            },
            CashFlow {
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                amount: 1700.0,
            },
        ];

        let xirr = calculate_xirr(&cash_flows);
        assert!(xirr.is_some());
        // Should be roughly 13-14% annualized
        let rate = xirr.unwrap();
        assert!(rate > 0.10 && rate < 0.20);
    }

    #[test]
    fn test_negative_return() {
        // Invest $1000, receive $900 one year later = -10% return
        let cash_flows = vec![
            CashFlow {
                date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
                amount: -1000.0,
            },
            CashFlow {
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                amount: 900.0,
            },
        ];

        let xirr = calculate_xirr(&cash_flows).unwrap();
        assert!((xirr - (-0.10)).abs() < 0.001);
    }

    #[test]
    fn test_insufficient_cash_flows() {
        let cash_flows = vec![CashFlow {
            date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            amount: -1000.0,
        }];

        assert!(calculate_xirr(&cash_flows).is_none());
    }

    #[test]
    fn test_same_sign_cash_flows() {
        // All outflows - no return
        let cash_flows = vec![
            CashFlow {
                date: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
                amount: -1000.0,
            },
            CashFlow {
                date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
                amount: -500.0,
            },
        ];

        assert!(calculate_xirr(&cash_flows).is_none());
    }
}
