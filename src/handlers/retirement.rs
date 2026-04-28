use askama::Template;
use axum::extract::{Path, Query, State};
use axum::response::{Html, Redirect};
use axum::{Form, Json};
use chrono::Utc;
use serde::Deserialize;

use crate::db::queries::retirement as db;
use crate::error::{AppError, AppResult, RenderHtml};
use crate::filters;
use crate::models::retirement::{
    RetirementChartData, RetirementProjection, Scenario, SimulateResponse,
};
use crate::models::Settings;
use crate::services::retirement::{
    build_chart_data, run_monte_carlo, savings_phase, withdrawal_phase, ProjectionInputs,
};
use crate::state::{AppState, JsManifest, PageBase};

fn today_year() -> i32 {
    Utc::now()
        .format("%Y")
        .to_string()
        .parse::<i32>()
        .unwrap_or(2025)
}

fn success_color_class(p: f64) -> &'static str {
    match p {
        p if p >= 0.80 => "text-emerald-600 dark:text-emerald-400",
        p if p >= 0.50 => "text-yellow-600 dark:text-yellow-400",
        _ => "text-red-600 dark:text-red-400",
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct RetirementPageParams {
    pub id: Option<String>,
}

#[derive(Template)]
#[template(path = "pages/retirement.html")]
pub struct RetirementTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub all_scenarios: Vec<Scenario>,
    pub projection: Option<RetirementProjection>,
    pub show_new_form: bool,
    pub current_net_worth_cents: i64,
    pub current_net_worth_formatted: String,
    pub form_scenario: Option<Scenario>,
    /// JSON-serialized slider state for the interactive compute UI.
    pub slider_state_json: String,
}

pub async fn index(
    State(state): State<AppState>,
    Query(params): Query<RetirementPageParams>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let PageBase {
        settings,
        icons,
        manifest,
        version,
        xsrf_token,
    } = state.page_base()?;

    let all_scenarios = db::list_scenarios(&conn)?;
    let current_net_worth_cents = db::get_current_net_worth_cents(&conn)?;
    let current_net_worth_formatted = filters::format_money_neutral(
        current_net_worth_cents,
        &settings.currency,
        &settings.locale,
    );

    let (projection, show_new_form, slider_state_json) = if all_scenarios.is_empty() {
        (None, true, "{}".to_string())
    } else {
        let scenario = if let Some(id) = &params.id {
            db::get_scenario(&conn, id)?
                .ok_or_else(|| AppError::NotFound(format!("Scenario {id} not found")))?
        } else {
            db::get_main_scenario(&conn)?
                .or_else(|| all_scenarios.first().cloned())
                .expect("list is non-empty")
        };

        let portfolio_cents = scenario
            .current_portfolio_override_cents
            .unwrap_or(current_net_worth_cents);
        let cost_basis_cents = scenario
            .deposits_cents
            .unwrap_or_else(|| db::get_total_invested_cents(&conn).unwrap_or(0));

        let year = today_year();
        let inputs =
            ProjectionInputs::from_scenario(&scenario, portfolio_cents, cost_basis_cents, year);

        let slider_json = build_slider_state_json(&scenario, portfolio_cents, cost_basis_cents);

        let proj = if let Some(inputs) = inputs {
            let mc = run_monte_carlo(&inputs, year)?;
            let savings = savings_phase(&inputs, year);
            let portfolio_at_retirement = savings
                .last()
                .map(|r| r.portfolio_end_cents)
                .unwrap_or(portfolio_cents);
            let years_savings = savings.len() as i64;
            let cost_basis_at_retirement =
                cost_basis_cents + inputs.annual_savings_cents * years_savings;
            let retirement_year = year + (inputs.retirement_age - inputs.current_age).ceil() as i32;
            let withdrawal = withdrawal_phase(
                &inputs,
                portfolio_at_retirement,
                cost_basis_at_retirement,
                retirement_year,
            );

            let success_prob = mc.success_probability;
            RetirementProjection {
                has_simulation: true,
                success_probability_display: Some(format!("{:.1}%", success_prob * 100.0)),
                success_color_class: success_color_class(success_prob),
                early_retirement_p10_display: mc
                    .early_retirement_p10_age
                    .map(|a| format!("Age {:.0}", a)),
                early_retirement_p50_display: mc
                    .early_retirement_p50_age
                    .map(|a| format!("Age {:.0}", a)),
                early_retirement_p90_display: mc
                    .early_retirement_p90_age
                    .map(|a| format!("Age {:.0}", a)),
                savings_rows: savings,
                withdrawal_rows: withdrawal,
                current_portfolio_cents: portfolio_cents,
                scenario,
            }
        } else {
            RetirementProjection {
                scenario,
                has_simulation: false,
                savings_rows: vec![],
                withdrawal_rows: vec![],
                success_probability_display: None,
                success_color_class: "text-neutral-900 dark:text-white",
                early_retirement_p10_display: None,
                early_retirement_p50_display: None,
                early_retirement_p90_display: None,
                current_portfolio_cents: portfolio_cents,
            }
        };

        (Some(proj), false, slider_json)
    };

    RetirementTemplate {
        title: "Retirement".into(),
        settings,
        icons,
        manifest,
        version,
        xsrf_token,
        all_scenarios,
        projection,
        show_new_form,
        current_net_worth_cents,
        current_net_worth_formatted,
        form_scenario: None,
        slider_state_json,
    }
    .render_html()
}

#[derive(Template)]
#[template(path = "pages/retirement_form.html")]
pub struct RetirementFormTemplate {
    pub title: String,
    pub settings: Settings,
    pub icons: filters::Icons,
    pub manifest: JsManifest,
    pub version: &'static str,
    pub xsrf_token: String,
    pub form_scenario: Option<Scenario>,
    pub current_net_worth_cents: i64,
    pub current_net_worth_formatted: String,
}

pub async fn new_form(State(state): State<AppState>) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let PageBase {
        settings,
        icons,
        manifest,
        version,
        xsrf_token,
    } = state.page_base()?;
    let current_net_worth_cents = db::get_current_net_worth_cents(&conn)?;
    let current_net_worth_formatted = filters::format_money_neutral(
        current_net_worth_cents,
        &settings.currency,
        &settings.locale,
    );

    RetirementFormTemplate {
        title: "New Scenario".into(),
        settings,
        icons,
        manifest,
        version,
        xsrf_token,
        form_scenario: None,
        current_net_worth_cents,
        current_net_worth_formatted,
    }
    .render_html()
}

pub async fn edit_form(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Html<String>> {
    let conn = state.db.get()?;
    let PageBase {
        settings,
        icons,
        manifest,
        version,
        xsrf_token,
    } = state.page_base()?;
    let scenario = db::get_scenario(&conn, &id)?
        .ok_or_else(|| AppError::NotFound(format!("Scenario {id} not found")))?;
    let current_net_worth_cents = db::get_current_net_worth_cents(&conn)?;
    let current_net_worth_formatted = filters::format_money_neutral(
        current_net_worth_cents,
        &settings.currency,
        &settings.locale,
    );

    RetirementFormTemplate {
        title: "Edit Scenario".into(),
        settings,
        icons,
        manifest,
        version,
        xsrf_token,
        form_scenario: Some(scenario),
        current_net_worth_cents,
        current_net_worth_formatted,
    }
    .render_html()
}

#[derive(Debug, Deserialize)]
pub struct ScenarioFormData {
    pub name: String,
    #[serde(default)]
    pub is_main: String,
    pub birthday: Option<String>,
    pub desired_retirement_age: Option<String>,
    pub marriage_status: Option<String>,
    pub current_portfolio_override: Option<String>,
    pub deposits: Option<String>,
    pub monthly_savings: Option<String>,
    pub assumed_roi: Option<String>,
    pub expected_inflation: Option<String>,
    pub monthly_living_costs: Option<String>,
    pub tax_rate: Option<String>,
    pub monthly_pension: Option<String>,
    pub monthly_barista_income: Option<String>,
    pub savings_growth_rate: Option<String>,
    pub official_retirement_age: Option<String>,
    pub life_expectancy: Option<String>,
}

fn parse_money_euros(s: Option<&str>) -> Option<i64> {
    s.and_then(|v| {
        let trimmed = v.trim().replace(',', ".");
        if trimmed.is_empty() {
            None
        } else {
            trimmed
                .parse::<f64>()
                .ok()
                .map(|f| (f * 100.0).round() as i64)
        }
    })
}

fn parse_pct(s: Option<&str>, default: f64) -> f64 {
    s.and_then(|v| {
        let trimmed = v.trim().replace(',', ".");
        if trimmed.is_empty() {
            None
        } else {
            trimmed.parse::<f64>().ok().map(|f| f / 100.0)
        }
    })
    .unwrap_or(default)
}

fn parse_int(s: Option<&str>, default: i64) -> i64 {
    s.and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(default)
}

fn form_to_scenario(form: ScenarioFormData, id: String, is_main_default: bool) -> Scenario {
    let birthday = form.birthday.filter(|s| !s.trim().is_empty());
    let is_main = form.is_main == "on" || is_main_default;
    Scenario {
        id,
        name: form.name,
        is_main,
        birthday,
        desired_retirement_age: form
            .desired_retirement_age
            .as_deref()
            .and_then(|s| s.trim().parse().ok()),
        marriage_status: form
            .marriage_status
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "single".into()),
        current_portfolio_override_cents: parse_money_euros(
            form.current_portfolio_override.as_deref(),
        ),
        monthly_savings_cents: parse_money_euros(form.monthly_savings.as_deref()),
        assumed_roi: parse_pct(form.assumed_roi.as_deref(), 0.07),
        expected_inflation: parse_pct(form.expected_inflation.as_deref(), 0.02),
        monthly_living_costs_cents: parse_money_euros(form.monthly_living_costs.as_deref()),
        tax_rate: parse_pct(form.tax_rate.as_deref(), 0.26375),
        monthly_pension_cents: parse_money_euros(form.monthly_pension.as_deref()),
        monthly_barista_income_cents: parse_money_euros(form.monthly_barista_income.as_deref()),
        savings_growth_rate: parse_pct(form.savings_growth_rate.as_deref(), 0.0),
        official_retirement_age: form
            .official_retirement_age
            .as_deref()
            .and_then(|s| s.trim().parse().ok()),
        life_expectancy: parse_int(form.life_expectancy.as_deref(), 95),
        deposits_cents: parse_money_euros(form.deposits.as_deref()),
        created_at: String::new(),
        updated_at: String::new(),
    }
}

pub async fn create(
    State(state): State<AppState>,
    Form(form): Form<ScenarioFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;
    let id = db::new_scenario_id();

    let is_first = db::list_scenarios(&conn)?.is_empty();
    let scenario = form_to_scenario(form, id.clone(), is_first);

    if scenario.is_main {
        conn.execute("UPDATE scenarios SET is_main = 0", [])?;
    }

    db::create_scenario(&conn, &scenario)?;
    Ok(Redirect::to(&format!("/retirement?id={id}")))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<ScenarioFormData>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;
    let scenario = form_to_scenario(form, id.clone(), false);

    if scenario.is_main {
        conn.execute("UPDATE scenarios SET is_main = 0", [])?;
    }

    db::update_scenario(&conn, &scenario)?;
    Ok(Redirect::to(&format!("/retirement?id={id}")))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<String>) -> AppResult<Redirect> {
    let conn = state.db.get()?;

    let was_main = db::get_scenario(&conn, &id)?
        .map(|s| s.is_main)
        .unwrap_or(false);

    db::delete_scenario(&conn, &id)?;

    if was_main {
        if let Err(e) = db::promote_new_main(&conn) {
            tracing::warn!("Failed to promote new main scenario: {e}");
        }
    }

    Ok(Redirect::to("/retirement"))
}

pub async fn set_main(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Redirect> {
    let conn = state.db.get()?;
    db::set_main_scenario(&conn, &id)?;
    Ok(Redirect::to(&format!("/retirement?id={id}")))
}

pub async fn chart_data(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<RetirementChartData>> {
    let conn = state.db.get()?;
    let scenario = db::get_scenario(&conn, &id)?
        .ok_or_else(|| AppError::NotFound(format!("Scenario {id} not found")))?;

    let current_net_worth_cents = db::get_current_net_worth_cents(&conn)?;
    let portfolio_cents = scenario
        .current_portfolio_override_cents
        .unwrap_or(current_net_worth_cents);
    let cost_basis_cents = scenario
        .deposits_cents
        .unwrap_or_else(|| db::get_total_invested_cents(&conn).unwrap_or(0));

    let year = today_year();
    let inputs =
        ProjectionInputs::from_scenario(&scenario, portfolio_cents, cost_basis_cents, year)
            .ok_or_else(|| {
                AppError::Validation("Scenario is missing birthday or retirement age".into())
            })?;

    let mc = run_monte_carlo(&inputs, year)?;
    Ok(Json(build_chart_data(&mc, &inputs, year)?))
}

/// POST /api/retirement/simulate — compute on the fly from raw field values.
/// Used by the slider UI for interactive exploration without saving.
#[derive(Debug, Deserialize)]
pub struct SimulateRequest {
    pub scenario_id: String,
    pub birthday: Option<String>,
    pub desired_retirement_age: Option<i64>,
    pub official_retirement_age: Option<i64>,
    pub life_expectancy: Option<i64>,
    /// Monthly savings in euros (will be converted to cents).
    pub monthly_savings: Option<f64>,
    pub monthly_living_costs: Option<f64>,
    pub monthly_pension: Option<f64>,
    pub monthly_barista_income: Option<f64>,
    /// Yearly savings growth as a percentage, e.g. 2.0 for 2%.
    pub savings_growth_rate: Option<f64>,
    /// ROI as a percentage, e.g. 7.0 for 7%.
    pub assumed_roi: Option<f64>,
    pub expected_inflation: Option<f64>,
    pub tax_rate: Option<f64>,
    pub current_portfolio_override: Option<f64>,
    pub deposits: Option<f64>,
}

pub async fn simulate(
    State(state): State<AppState>,
    Json(req): Json<SimulateRequest>,
) -> AppResult<Json<SimulateResponse>> {
    let conn = state.db.get()?;
    let year = today_year();

    let current_net_worth_cents = db::get_current_net_worth_cents(&conn)?;
    let portfolio_cents = req
        .current_portfolio_override
        .map(|e| (e * 100.0).round() as i64)
        .unwrap_or_else(|| {
            db::get_scenario(&conn, &req.scenario_id)
                .ok()
                .flatten()
                .and_then(|s| s.current_portfolio_override_cents)
                .unwrap_or(current_net_worth_cents)
        });

    let cost_basis_cents = req
        .deposits
        .map(|e| (e * 100.0).round() as i64)
        .unwrap_or_else(|| db::get_total_invested_cents(&conn).unwrap_or(0));

    // Build a temporary Scenario from the request values.
    let scenario = Scenario {
        id: req.scenario_id.clone(),
        name: String::new(),
        is_main: false,
        birthday: req.birthday,
        desired_retirement_age: req.desired_retirement_age,
        marriage_status: "single".into(),
        current_portfolio_override_cents: None,
        monthly_savings_cents: req.monthly_savings.map(|e| (e * 100.0).round() as i64),
        assumed_roi: req.assumed_roi.map(|p| p / 100.0).unwrap_or(0.07),
        expected_inflation: req.expected_inflation.map(|p| p / 100.0).unwrap_or(0.02),
        monthly_living_costs_cents: req.monthly_living_costs.map(|e| (e * 100.0).round() as i64),
        tax_rate: req.tax_rate.map(|p| p / 100.0).unwrap_or(0.26375),
        monthly_pension_cents: req.monthly_pension.map(|e| (e * 100.0).round() as i64),
        monthly_barista_income_cents: req
            .monthly_barista_income
            .map(|e| (e * 100.0).round() as i64),
        savings_growth_rate: req.savings_growth_rate.map(|p| p / 100.0).unwrap_or(0.0),
        official_retirement_age: req.official_retirement_age,
        life_expectancy: req.life_expectancy.unwrap_or(95),
        deposits_cents: None,
        created_at: String::new(),
        updated_at: String::new(),
    };

    let inputs =
        ProjectionInputs::from_scenario(&scenario, portfolio_cents, cost_basis_cents, year)
            .ok_or_else(|| {
                AppError::Validation("Scenario is missing birthday or retirement age".into())
            })?;

    let mc = run_monte_carlo(&inputs, year)?;
    let chart = build_chart_data(&mc, &inputs, year)?;

    let success_prob = mc.success_probability;
    Ok(Json(SimulateResponse {
        success_probability_display: Some(format!("{:.1}%", success_prob * 100.0)),
        success_color_class: success_color_class(success_prob),
        early_retirement_p10_display: mc.early_retirement_p10_age.map(|a| format!("Age {:.0}", a)),
        early_retirement_p50_display: mc.early_retirement_p50_age.map(|a| format!("Age {:.0}", a)),
        early_retirement_p90_display: mc.early_retirement_p90_age.map(|a| format!("Age {:.0}", a)),
        chart,
    }))
}

/// Serialize the scenario's current values as a JSON string for the slider UI.
fn build_slider_state_json(
    scenario: &Scenario,
    portfolio_cents: i64,
    cost_basis_cents: i64,
) -> String {
    serde_json::json!({
        "scenario_id": scenario.id,
        "birthday": scenario.birthday,
        "desired_retirement_age": scenario.desired_retirement_age,
        "official_retirement_age": scenario.official_retirement_age,
        "life_expectancy": scenario.life_expectancy,
        "monthly_savings": scenario.monthly_savings_cents.unwrap_or(0) as f64 / 100.0,
        "monthly_living_costs": scenario.monthly_living_costs_cents.unwrap_or(0) as f64 / 100.0,
        "monthly_pension": scenario.monthly_pension_cents.unwrap_or(0) as f64 / 100.0,
        "monthly_barista_income": scenario.monthly_barista_income_cents.unwrap_or(0) as f64 / 100.0,
        "savings_growth_rate": scenario.savings_growth_rate * 100.0,
        "assumed_roi": scenario.assumed_roi * 100.0,
        "expected_inflation": scenario.expected_inflation * 100.0,
        "tax_rate": scenario.tax_rate * 100.0,
        "current_portfolio_override": portfolio_cents as f64 / 100.0,
        "deposits": cost_basis_cents as f64 / 100.0,
    })
    .to_string()
}
