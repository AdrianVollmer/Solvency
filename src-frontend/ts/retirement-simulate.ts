// Slider-driven interactive simulation for the retirement calculator.
// Reads scenario state from the embedded JSON tag, sends PATCH requests to
// /api/retirement/simulate on slider change, and updates the hero stats + chart.

interface ScenarioState {
  scenario_id: string;
  birthday: string | null;
  desired_retirement_age: number | null;
  official_retirement_age: number | null;
  life_expectancy: number;
  monthly_savings: number;
  monthly_living_costs: number;
  monthly_pension: number;
  monthly_barista_income: number;
  savings_growth_rate: number;
  assumed_roi: number;
  expected_inflation: number;
  tax_rate: number;
  current_portfolio_override: number;
  deposits: number;
}

interface SimulateResponse {
  success_probability_display: string | null;
  success_color_class: string;
  early_retirement_p10_display: string | null;
  early_retirement_p50_display: string | null;
  early_retirement_p90_display: string | null;
  chart: object;
}

type SliderFormat = "age" | "money" | "percent";

let scenarioState: ScenarioState | null = null;
let activeField: string | null = null;
let activeFormat: SliderFormat = "age";
let debounceTimer: ReturnType<typeof setTimeout> | null = null;
let currency = "EUR";
let locale = "de-DE";

// Money fields store raw cents in the slider; these need /100 conversion for display.
const MONEY_FIELDS = new Set([
  "monthly_savings",
  "monthly_living_costs",
  "monthly_pension",
  "monthly_barista_income",
  "current_portfolio_override",
]);

function getCurrencySymbol(cur: string): string {
  const symbols: Record<string, string> = {
    USD: "$", EUR: "€", GBP: "£", JPY: "¥", CHF: "CHF ",
    CAD: "C$", AUD: "A$", SEK: "kr ", NOK: "kr ", DKK: "kr ",
  };
  return symbols[cur.toUpperCase()] || cur + " ";
}

function formatSliderValue(value: number, format: SliderFormat): string {
  if (format === "age") return `Age ${Math.round(value)}`;
  if (format === "percent") return `${value.toFixed(2)}%`;
  // money: value is in cents
  const euros = value / 100;
  const sym = getCurrencySymbol(currency);
  return sym + euros.toLocaleString(locale, { minimumFractionDigits: 0, maximumFractionDigits: 0 });
}

function formatParamDisplay(value: number, format: SliderFormat): string {
  if (format === "age") return `Age ${Math.round(value)}`;
  if (format === "percent") return `${value.toFixed(2)}%`;
  const euros = value / 100;
  const sym = getCurrencySymbol(currency);
  return sym + euros.toLocaleString(locale, { minimumFractionDigits: 0, maximumFractionDigits: 0 });
}

// Called from inline onclick in the template.
// currentValue: for money fields, this is cents; for age, integer; for percent, percentage.
(window as any).openSlider = function (
  field: string,
  currentValue: number,
  min: number,
  max: number,
  step: number,
  label: string,
  format: SliderFormat
) {
  if (!scenarioState) return;

  activeField = field;
  activeFormat = format;

  const bar = document.getElementById("slider-bar");
  const labelEl = document.getElementById("slider-field-label");
  const input = document.getElementById("slider-input") as HTMLInputElement | null;
  const valueEl = document.getElementById("slider-value-display");

  if (!bar || !labelEl || !input || !valueEl) return;

  labelEl.textContent = label;
  input.min = String(min);
  input.max = String(max);
  input.step = String(step);
  input.value = String(currentValue);
  valueEl.textContent = formatSliderValue(currentValue, format);

  bar.classList.remove("hidden");
  input.focus();
};

(window as any).closeSlider = function () {
  const bar = document.getElementById("slider-bar");
  bar?.classList.add("hidden");
  activeField = null;
};

(window as any).onSliderInput = function (rawValue: string) {
  if (!activeField || !scenarioState) return;

  const value = parseFloat(rawValue);

  // Update the inline value display
  const valueEl = document.getElementById("slider-value-display");
  if (valueEl) valueEl.textContent = formatSliderValue(value, activeFormat);

  // Update the param card value display
  const paramEl = document.getElementById(`val-${activeField}`);
  if (paramEl) paramEl.textContent = formatParamDisplay(value, activeFormat);

  // Write the new value into the state (converting cents → euros for API)
  if (MONEY_FIELDS.has(activeField)) {
    (scenarioState as any)[activeField] = value / 100;
  } else if (activeField === "desired_retirement_age" || activeField === "official_retirement_age" || activeField === "life_expectancy") {
    (scenarioState as any)[activeField] = Math.round(value);
  } else {
    (scenarioState as any)[activeField] = value;
  }

  // Debounce the API call
  if (debounceTimer !== null) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(runSimulate, 220);
};

function getXsrfToken(): string {
  const meta = document.querySelector('meta[name="xsrf-token"]');
  return meta?.getAttribute("content") ?? "";
}

async function runSimulate() {
  if (!scenarioState) return;

  try {
    const resp = await fetch("/api/retirement/simulate", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-XSRF-Token": getXsrfToken(),
      },
      body: JSON.stringify(scenarioState),
    });
    if (!resp.ok) return;

    const data: SimulateResponse = await resp.json();
    updateHeroStats(data);
    updateChart(data.chart);
  } catch {
    // Silently ignore network errors during slider interaction
  }
}

function updateHeroStats(data: SimulateResponse) {
  const probEl = document.getElementById("hero-success-probability");
  if (probEl && data.success_probability_display) {
    probEl.textContent = data.success_probability_display;
    // Update colour class: strip existing text-* classes and apply new one
    probEl.className = probEl.className
      .split(" ")
      .filter((c) => !c.startsWith("text-emerald") && !c.startsWith("text-yellow") && !c.startsWith("text-red"))
      .join(" ");
    for (const cls of data.success_color_class.split(" ")) {
      probEl.classList.add(cls);
    }
  }

  const p10El = document.getElementById("hero-p10");
  if (p10El && data.early_retirement_p10_display)
    p10El.textContent = data.early_retirement_p10_display;

  const p50El = document.getElementById("hero-p50");
  if (p50El && data.early_retirement_p50_display)
    p50El.textContent = data.early_retirement_p50_display;

  const p90El = document.getElementById("hero-p90");
  if (p90El && data.early_retirement_p90_display)
    p90El.textContent = data.early_retirement_p90_display;
}

function updateChart(chartData: object) {
  const updateFn = (window as any).updateRetirementChart;
  if (typeof updateFn === "function") updateFn(chartData);
}

document.addEventListener("DOMContentLoaded", () => {
  // Read currency/locale from the chart container (same source as the chart TS).
  const chartContainer = document.getElementById("retirement-chart");
  if (chartContainer) {
    currency = chartContainer.dataset.currency ?? "EUR";
    locale = chartContainer.dataset.locale ?? "de-DE";
  }

  // Load the embedded scenario state.
  const stateTag = document.getElementById("retirement-state");
  if (stateTag) {
    try {
      scenarioState = JSON.parse(stateTag.textContent ?? "{}");
    } catch {
      // No state available (no scenarios yet)
    }
  }
});
