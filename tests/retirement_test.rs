mod common;

use axum::http::StatusCode;
use common::TestClient;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct RetirementChartData {
    years: Vec<i32>,
    p10: Vec<i64>,
    p50: Vec<i64>,
    p90: Vec<i64>,
    deterministic: Vec<i64>,
}

/// A scenario with no starting portfolio; living costs exceed what savings alone can fund.
fn tight_scenario_form<'a>(name: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("name", name),
        ("birthday", "1985-01-01"),
        ("desired_retirement_age", "60"),
        ("monthly_savings", "500"),      // low savings
        ("monthly_living_costs", "5000"), // high costs
        ("official_retirement_age", "67"),
        ("life_expectancy", "90"),
        ("assumed_roi", "7"),
        ("expected_inflation", "2"),
        ("tax_rate", "26.375"),
        // No current_portfolio_override — defaults to DB net worth (0 in test env)
    ]
}

fn scenario_form<'a>(name: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![
        ("name", name),
        ("birthday", "1985-01-01"),
        ("desired_retirement_age", "65"),
        ("monthly_savings", "2000"),
        ("monthly_living_costs", "3000"),
        ("official_retirement_age", "67"),
        ("life_expectancy", "95"),
        ("assumed_roi", "7"),
        ("expected_inflation", "2"),
        ("tax_rate", "26.375"),
    ]
}

/// Empty DB: GET /retirement shows the new-scenario form.
#[tokio::test]
async fn test_retirement_page_no_scenarios_shows_form() {
    let client = TestClient::new();
    let (status, body) = client.get("/retirement").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("name=\"name\""),
        "Expected scenario form input, got: {}",
        &body[..body.len().min(500)]
    );
}

/// Create a scenario: POST /retirement/create redirects.
#[tokio::test]
async fn test_create_scenario_redirects() {
    let client = TestClient::new();
    let (status, _) = client
        .post_form("/retirement/create", &scenario_form("Test Plan"))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Expected redirect after create");
}

/// Created scenario appears in page dropdown.
#[tokio::test]
async fn test_created_scenario_visible_in_page() {
    let client = TestClient::new();
    client
        .post_form("/retirement/create", &scenario_form("My Plan"))
        .await;

    let (status, body) = client.get("/retirement").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("My Plan"),
        "Scenario name not found: {}",
        &body[..body.len().min(1000)]
    );
}

/// First scenario auto-becomes main.
#[tokio::test]
async fn test_first_scenario_is_main() {
    let client = TestClient::new();
    client
        .post_form("/retirement/create", &scenario_form("Plan A"))
        .await;

    let (_, body) = client.get("/retirement").await;
    assert!(
        body.contains("(main)"),
        "Expected (main) label: {}",
        &body[..body.len().min(1000)]
    );
}

/// Only one scenario can be main at a time.
#[tokio::test]
async fn test_only_one_main_scenario() {
    let client = TestClient::new();

    let mut form_a = scenario_form("Plan A");
    form_a.push(("is_main", "on"));
    client.post_form("/retirement/create", &form_a).await;

    let mut form_b = scenario_form("Plan B");
    form_b.push(("is_main", "on"));
    client.post_form("/retirement/create", &form_b).await;

    let (_, body) = client.get("/retirement").await;
    let main_count = body.matches("(main)").count();
    assert_eq!(
        main_count, 1,
        "Expected exactly one (main) label, found {main_count}"
    );
}

/// After creating with complete inputs, success probability is shown.
#[tokio::test]
async fn test_simulation_runs_and_probability_shown() {
    let client = TestClient::new();
    client
        .post_form("/retirement/create", &scenario_form("Sim Test"))
        .await;

    let (status, body) = client.get("/retirement").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Success Probability"),
        "Expected success probability stat: {}",
        &body[..body.len().min(2000)]
    );
}

/// Savings phase table is rendered.
#[tokio::test]
async fn test_savings_table_rendered() {
    let client = TestClient::new();
    client
        .post_form("/retirement/create", &scenario_form("Table Test"))
        .await;

    let (_, body) = client.get("/retirement").await;
    assert!(
        body.contains("Savings Phase"),
        "Expected Savings Phase table: {}",
        &body[..body.len().min(2000)]
    );
    assert!(
        body.contains("Withdrawal Phase"),
        "Expected Withdrawal Phase table"
    );
}

/// Deleting the main scenario auto-promotes the next one.
#[tokio::test]
async fn test_delete_main_promotes_next() {
    let client = TestClient::new();

    client
        .post_form("/retirement/create", &scenario_form("Alpha"))
        .await;
    client
        .post_form("/retirement/create", &scenario_form("Beta"))
        .await;

    // Find the id of the current main scenario from the page
    let (_, body) = client.get("/retirement").await;
    let search = "data-scenario-id=\"";
    let id_start = body
        .find(search)
        .expect("no data-scenario-id attribute found")
        + search.len();
    let id_end = body[id_start..]
        .find('"')
        .map(|i| id_start + i)
        .unwrap_or(id_start + 36);
    let main_id = body[id_start..id_end].to_string();

    let (status, _) = client
        .post_form(&format!("/retirement/{main_id}/delete"), &[])
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    // Page should still load and show something
    let (status, body) = client.get("/retirement").await;
    assert_eq!(status, StatusCode::OK);
    // Either Alpha or Beta should still be visible
    assert!(
        body.contains("Alpha") || body.contains("Beta"),
        "Expected a scenario to remain: {}",
        &body[..body.len().min(1000)]
    );
}

/// Chart data API returns valid JSON.
#[tokio::test]
async fn test_chart_data_api() {
    let client = TestClient::new();

    client
        .post_form("/retirement/create", &scenario_form("Chart Test"))
        .await;

    // Get scenario id from the retirement page
    let (_, body) = client.get("/retirement").await;
    let search = "data-scenario-id=\"";
    let id_start = body
        .find(search)
        .expect("no data-scenario-id attribute found")
        + search.len();
    let id_end = body[id_start..]
        .find('"')
        .map(|i| id_start + i)
        .unwrap_or(id_start + 36);
    let scenario_id = body[id_start..id_end].to_string();

    let (status, data): (_, Option<RetirementChartData>) = client
        .get_json(&format!("/api/retirement/{scenario_id}/chart"))
        .await;

    assert_eq!(status, StatusCode::OK);
    let chart = data.expect("Failed to parse chart JSON");
    assert!(!chart.years.is_empty(), "Chart years must not be empty");
    assert_eq!(
        chart.years.len(),
        chart.p50.len(),
        "years and p50 length mismatch"
    );
    assert_eq!(
        chart.years.len(),
        chart.deterministic.len(),
        "years and deterministic length mismatch"
    );
}

/// set-main endpoint changes which scenario is main.
#[tokio::test]
async fn test_set_main() {
    let client = TestClient::new();

    client
        .post_form("/retirement/create", &scenario_form("First"))
        .await;
    client
        .post_form("/retirement/create", &scenario_form("Second"))
        .await;

    // Get second scenario's id
    let (_, body) = client.get("/retirement").await;
    // Find "Second" in the select and get its value (Askama renders option
    // content on a new indented line, so ">Second<" won't match literally)
    let second_pos = body.find("Second").expect("Second scenario not found");
    let value_search = "value=\"";
    // Walk backwards from second_pos to find the preceding option value
    let before = &body[..second_pos];
    let val_start = before
        .rfind(value_search)
        .expect("no value= before Second")
        + value_search.len();
    let val_end = before[val_start..]
        .find('"')
        .map(|i| val_start + i)
        .unwrap_or(val_start + 36);
    let second_id = before[val_start..val_end].to_string();

    let (status, _) = client
        .post_form(&format!("/retirement/{second_id}/set-main"), &[])
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);

    let (_, body) = client.get(&format!("/retirement?id={second_id}")).await;
    // "Second" should now be main - the set-main button should be gone
    assert!(
        !body.contains("Set as Main"),
        "Expected 'Set as Main' button to be absent for the current main scenario"
    );
}

/// Success probability for a tight scenario must be well below 100%.
/// This catches the lognormal return bug where every simulation grew unboundedly.
#[tokio::test]
async fn test_success_probability_not_always_100_percent() {
    let client = TestClient::new();
    client
        .post_form("/retirement/create", &tight_scenario_form("Tight"))
        .await;

    let (_, body) = client.get("/retirement").await;
    // Extract the probability text — it appears as e.g. "34.5%"
    assert!(
        body.contains("Success Probability"),
        "Success Probability stat must be present"
    );
    // The value must NOT be "100.0%" — a tight scenario cannot always succeed
    assert!(
        !body.contains(">100.0%<"),
        "Success Probability should not be 100% for a tight scenario: {:?}",
        body.find("Success Probability").map(|i| &body[i..i.min(body.len()).min(i + 200)])
    );
}

/// Chart p50 values must be in a plausible order of magnitude (not trillions).
/// The old lognormal bug caused the median to reach 10^13+ within a few years.
#[tokio::test]
async fn test_chart_p50_values_are_plausible() {
    let client = TestClient::new();
    client
        .post_form("/retirement/create", &scenario_form("Chart Sanity"))
        .await;

    let (_, body) = client.get("/retirement").await;
    let search = "data-scenario-id=\"";
    let id_start = body.find(search).expect("no data-scenario-id") + search.len();
    let id_end = body[id_start..].find('"').map(|i| id_start + i).unwrap_or(id_start + 36);
    let scenario_id = body[id_start..id_end].to_string();

    let (status, data): (_, Option<RetirementChartData>) = client
        .get_json(&format!("/api/retirement/{scenario_id}/chart"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let chart = data.expect("failed to parse chart JSON");

    // All p50 values must be < 100 billion euros (10^13 cents).
    // The bug caused values to exceed 10^15+ cents within 10 years.
    let limit = 100_000_000_000_00i64; // 100 billion euros in cents
    for (i, &v) in chart.p50.iter().enumerate() {
        assert!(
            v <= limit,
            "p50[{i}] = {v} cents exceeds 100 billion euros — likely the lognormal return bug"
        );
    }
}

/// p10 band must be ≤ p50, p50 ≤ p90 at every point in the chart.
#[tokio::test]
async fn test_chart_percentile_bands_are_ordered() {
    let client = TestClient::new();
    client
        .post_form("/retirement/create", &scenario_form("Band Order"))
        .await;

    let (_, body) = client.get("/retirement").await;
    let search = "data-scenario-id=\"";
    let id_start = body.find(search).expect("no data-scenario-id") + search.len();
    let id_end = body[id_start..].find('"').map(|i| id_start + i).unwrap_or(id_start + 36);
    let scenario_id = body[id_start..id_end].to_string();

    let (_, data): (_, Option<RetirementChartData>) = client
        .get_json(&format!("/api/retirement/{scenario_id}/chart"))
        .await;
    let chart = data.expect("failed to parse chart JSON");

    for (i, ((&p10v, &p50v), &p90v)) in chart
        .p10
        .iter()
        .zip(chart.p50.iter())
        .zip(chart.p90.iter())
        .enumerate()
    {
        assert!(p10v <= p50v, "year {i}: p10 ({p10v}) > p50 ({p50v})");
        assert!(p50v <= p90v, "year {i}: p50 ({p50v}) > p90 ({p90v})");
    }
}

/// Savings table must show Gain column, not Tax or Net Return.
#[tokio::test]
async fn test_savings_table_columns() {
    let client = TestClient::new();
    client
        .post_form("/retirement/create", &scenario_form("Col Test"))
        .await;
    let (_, body) = client.get("/retirement").await;

    // Isolate the Savings Phase section (ends before Withdrawal Phase)
    let savings_start = body.find("Savings Phase").expect("Savings Phase missing");
    let savings_section = match body[savings_start..].find("Withdrawal Phase") {
        Some(rel) => &body[savings_start..savings_start + rel],
        None => &body[savings_start..],
    };

    assert!(
        !savings_section.contains(">Tax<"),
        "Tax column must not appear in savings table"
    );
    assert!(
        !savings_section.contains(">Net Return<"),
        "Net Return must not appear in savings table"
    );
    assert!(
        savings_section.contains(">Gain<"),
        "Gain column must be present in savings table"
    );
}
