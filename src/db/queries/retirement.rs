use rusqlite::{params, Connection, OptionalExtension};
use tracing::{info, warn};
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::retirement::Scenario;

fn row_to_scenario(row: &rusqlite::Row) -> rusqlite::Result<Scenario> {
    Ok(Scenario {
        id: row.get(0)?,
        name: row.get(1)?,
        is_main: row.get::<_, i64>(2)? != 0,
        birthday: row.get(3)?,
        desired_retirement_age: row.get(4)?,
        marriage_status: row.get(5)?,
        current_portfolio_override_cents: row.get(6)?,
        monthly_savings_cents: row.get(7)?,
        assumed_roi: row.get(8)?,
        expected_inflation: row.get(9)?,
        monthly_living_costs_cents: row.get(10)?,
        tax_rate: row.get(11)?,
        monthly_pension_cents: row.get(12)?,
        official_retirement_age: row.get(13)?,
        life_expectancy: row.get(14)?,
        deposits_cents: row.get(15)?,
        monthly_barista_income_cents: row.get(16)?,
        savings_growth_rate: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}

const SELECT_COLS: &str = "id, name, is_main, birthday, desired_retirement_age, \
    marriage_status, current_portfolio_override_cents, monthly_savings_cents, \
    assumed_roi, expected_inflation, monthly_living_costs_cents, tax_rate, \
    monthly_pension_cents, official_retirement_age, life_expectancy, deposits_cents, \
    monthly_barista_income_cents, savings_growth_rate, created_at, updated_at";

pub fn list_scenarios(conn: &Connection) -> AppResult<Vec<Scenario>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} FROM scenarios ORDER BY is_main DESC, name ASC"
    ))?;
    let rows = stmt
        .query_map([], row_to_scenario)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_scenario(conn: &Connection, id: &str) -> AppResult<Option<Scenario>> {
    Ok(conn
        .query_row(
            &format!("SELECT {SELECT_COLS} FROM scenarios WHERE id = ?"),
            [id],
            row_to_scenario,
        )
        .optional()?)
}

pub fn get_main_scenario(conn: &Connection) -> AppResult<Option<Scenario>> {
    Ok(conn
        .query_row(
            &format!("SELECT {SELECT_COLS} FROM scenarios WHERE is_main = 1 LIMIT 1"),
            [],
            row_to_scenario,
        )
        .optional()?)
}

pub fn create_scenario(conn: &Connection, scenario: &Scenario) -> AppResult<()> {
    conn.execute(
        "INSERT INTO scenarios (id, name, is_main, birthday, desired_retirement_age, \
         marriage_status, current_portfolio_override_cents, monthly_savings_cents, \
         assumed_roi, expected_inflation, monthly_living_costs_cents, tax_rate, \
         monthly_pension_cents, official_retirement_age, life_expectancy, deposits_cents, \
         monthly_barista_income_cents, savings_growth_rate) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            scenario.id,
            scenario.name,
            scenario.is_main as i64,
            scenario.birthday,
            scenario.desired_retirement_age,
            scenario.marriage_status,
            scenario.current_portfolio_override_cents,
            scenario.monthly_savings_cents,
            scenario.assumed_roi,
            scenario.expected_inflation,
            scenario.monthly_living_costs_cents,
            scenario.tax_rate,
            scenario.monthly_pension_cents,
            scenario.official_retirement_age,
            scenario.life_expectancy,
            scenario.deposits_cents,
            scenario.monthly_barista_income_cents,
            scenario.savings_growth_rate,
        ],
    )?;
    info!(id = %scenario.id, name = %scenario.name, "Created scenario");
    Ok(())
}

pub fn update_scenario(conn: &Connection, scenario: &Scenario) -> AppResult<()> {
    conn.execute(
        "UPDATE scenarios SET name = ?, is_main = ?, birthday = ?, desired_retirement_age = ?, \
         marriage_status = ?, current_portfolio_override_cents = ?, monthly_savings_cents = ?, \
         assumed_roi = ?, expected_inflation = ?, monthly_living_costs_cents = ?, \
         tax_rate = ?, monthly_pension_cents = ?, official_retirement_age = ?, \
         life_expectancy = ?, deposits_cents = ?, monthly_barista_income_cents = ?, \
         savings_growth_rate = ?, updated_at = datetime('now') WHERE id = ?",
        params![
            scenario.name,
            scenario.is_main as i64,
            scenario.birthday,
            scenario.desired_retirement_age,
            scenario.marriage_status,
            scenario.current_portfolio_override_cents,
            scenario.monthly_savings_cents,
            scenario.assumed_roi,
            scenario.expected_inflation,
            scenario.monthly_living_costs_cents,
            scenario.tax_rate,
            scenario.monthly_pension_cents,
            scenario.official_retirement_age,
            scenario.life_expectancy,
            scenario.deposits_cents,
            scenario.monthly_barista_income_cents,
            scenario.savings_growth_rate,
            scenario.id,
        ],
    )?;
    info!(id = %scenario.id, name = %scenario.name, "Updated scenario");
    Ok(())
}

pub fn delete_scenario(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM scenarios WHERE id = ?", [id])?;
    warn!(id = %id, "Deleted scenario");
    Ok(())
}

/// Atomically set is_main=1 on `id`, 0 on all others.
pub fn set_main_scenario(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("UPDATE scenarios SET is_main = 0", [])?;
    conn.execute("UPDATE scenarios SET is_main = 1 WHERE id = ?", [id])?;
    info!(id = %id, "Set main scenario");
    Ok(())
}

/// After deleting the main scenario, promote the first remaining one (alphabetically).
pub fn promote_new_main(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "UPDATE scenarios SET is_main = 1 \
         WHERE id = (SELECT id FROM scenarios ORDER BY name ASC LIMIT 1)",
        [],
    )?;
    Ok(())
}


/// Returns the current net worth in cents by delegating to the net worth service.
pub fn get_current_net_worth_cents(conn: &Connection) -> AppResult<i64> {
    use crate::services::net_worth::calculate_net_worth_history;
    let summary = calculate_net_worth_history(conn)?;
    Ok(summary.current_net_worth_cents)
}

/// Returns the total amount invested (net cost basis) from trading activity history.
/// Sums BUY cost (quantity × unit_price + fees) minus SELL proceeds.
/// Returns 0 if no trading history exists.
pub fn get_total_invested_cents(conn: &Connection) -> AppResult<i64> {
    let result: i64 = conn.query_row(
        "SELECT COALESCE(SUM(
            CASE
                WHEN activity_type = 'BUY'  THEN CAST(quantity * unit_price_cents AS INTEGER) + fee_cents
                WHEN activity_type = 'SELL' THEN -(CAST(quantity * unit_price_cents AS INTEGER) - fee_cents)
                ELSE 0
            END
         ), 0) FROM trading_activities",
        [],
        |row| row.get(0),
    )?;
    Ok(result.max(0))
}

pub fn new_scenario_id() -> String {
    Uuid::new_v4().to_string()
}
