declare const echarts: any;

import { formatMoney, isDarkMode, getTheme } from "./utils";

interface RetirementChartData {
  years: number[];
  p10: number[];
  p25: number[];
  p50: number[];
  p75: number[];
  p90: number[];
  deterministic: number[];
  retirement_year: number | null;
  pension_year: number | null;
  life_expectancy_year: number | null;
}

type ChartMode = "real" | "nominal";

let retirementChart: any = null;
let currentMode: ChartMode = "real";

/** Convert real series to nominal by multiplying each point by (1+inflation)^i. */
function toNominal(real: number[], inflation: number): number[] {
  return real.map((v, i) => Math.round(v * Math.pow(1 + inflation, i + 1)));
}

function buildMarkLines(data: RetirementChartData): any[] {
  const lines: any[] = [];
  if (data.retirement_year) {
    lines.push({
      xAxis: String(data.retirement_year),
      label: {
        formatter: "Retirement",
        position: "insideStartTop",
        fontSize: 11,
      },
      lineStyle: { color: "#3b82f6", type: "dashed", width: 1.5 },
    });
  }
  if (data.pension_year && data.pension_year !== data.retirement_year) {
    lines.push({
      xAxis: String(data.pension_year),
      label: {
        formatter: "Pension",
        position: "insideStartBottom",
        fontSize: 11,
      },
      lineStyle: { color: "#22c55e", type: "dashed", width: 1.5 },
    });
  }
  if (data.life_expectancy_year) {
    lines.push({
      xAxis: String(data.life_expectancy_year),
      label: {
        formatter: "Life exp.",
        position: "insideEndTop",
        fontSize: 11,
      },
      lineStyle: { color: "#ef4444", type: "dotted", width: 1.5 },
    });
  }
  return lines;
}

function buildOption(
  p10: number[],
  p25: number[],
  p50: number[],
  p75: number[],
  p90: number[],
  deterministic: number[],
  years: string[],
  markLines: any[],
  currency: string,
  locale: string
): object {
  return {
    backgroundColor: "transparent",
    tooltip: {
      trigger: "axis",
      axisPointer: { type: "line" },
      formatter: (params: any[]) => {
        const year = params[0]?.axisValue ?? "";
        const filtered = params.filter(
          (p: any) =>
            p.seriesName !== "_p10_base" && p.seriesName !== "_p25_base"
        );
        const lines = filtered.map((p: any) => {
          const cents =
            p.seriesName === "P25–P75 band" || p.seriesName === "P10–P90 band"
              ? null
              : p.value;
          const val =
            cents !== null ? formatMoney(Number(cents), currency, locale, 0) : "";
          return val
            ? `<div style="display:flex;justify-content:space-between;gap:16px"><span>${p.marker}${p.seriesName}</span><span style="font-weight:600">${val}</span></div>`
            : "";
        });
        return `<strong>${year}</strong><br>${lines.filter(Boolean).join("")}`;
      },
    },
    legend: {
      data: ["P50 Median", "Deterministic", "P25–P75 band", "P10–P90 band"],
      bottom: 4,
      textStyle: { fontSize: 11 },
    },
    grid: {
      left: "2%",
      right: "2%",
      bottom: "12%",
      top: "4%",
      containLabel: true,
    },
    xAxis: {
      type: "category",
      data: years,
      boundaryGap: false,
      axisLabel: { fontSize: 11 },
    },
    yAxis: {
      type: "value",
      axisLabel: {
        fontSize: 11,
        formatter: (v: number) => formatMoney(v, currency, locale, 0),
      },
    },
    series: [
      // Invisible base for P10–P90 band
      {
        name: "_p10_base",
        type: "line",
        data: p10,
        lineStyle: { opacity: 0 },
        symbol: "none",
        stack: "band_outer",
        silent: true,
        legendHoverLink: false,
        tooltip: { show: false },
        showInLegend: false,
      },
      // P90 - P10 fill area
      {
        name: "P10–P90 band",
        type: "line",
        data: p90.map((v, i) => v - p10[i]),
        lineStyle: { opacity: 0 },
        symbol: "none",
        stack: "band_outer",
        areaStyle: { opacity: 0.12, color: "#3b82f6" },
        silent: true,
      },
      // Invisible base for P25–P75 band
      {
        name: "_p25_base",
        type: "line",
        data: p25,
        lineStyle: { opacity: 0 },
        symbol: "none",
        stack: "band_inner",
        silent: true,
        legendHoverLink: false,
        tooltip: { show: false },
        showInLegend: false,
      },
      // P75 - P25 fill area
      {
        name: "P25–P75 band",
        type: "line",
        data: p75.map((v, i) => v - p25[i]),
        lineStyle: { opacity: 0 },
        symbol: "none",
        stack: "band_inner",
        areaStyle: { opacity: 0.25, color: "#3b82f6" },
        silent: true,
      },
      // Median line
      {
        name: "P50 Median",
        type: "line",
        data: p50,
        lineStyle: { color: "#3b82f6", width: 2 },
        symbol: "none",
        markLine:
          markLines.length > 0
            ? { silent: true, symbol: "none", data: markLines }
            : undefined,
      },
      // Deterministic dashed
      {
        name: "Deterministic",
        type: "line",
        data: deterministic,
        lineStyle: { color: "#f59e0b", width: 1.5, type: "dashed" },
        symbol: "none",
      },
    ],
  };
}

// Exposed globally so the inline onclick handlers in the template can call it.
(window as any).setChartMode = function (mode: ChartMode) {
  currentMode = mode;

  const realBtn = document.getElementById("chart-mode-real");
  const nominalBtn = document.getElementById("chart-mode-nominal");
  const activeClass =
    "px-3 py-1.5 bg-neutral-100 dark:bg-neutral-700 text-neutral-900 dark:text-white";
  const inactiveClass =
    "px-3 py-1.5 text-neutral-500 dark:text-neutral-400 hover:bg-neutral-50 dark:hover:bg-neutral-700/50";

  if (realBtn && nominalBtn) {
    realBtn.className = mode === "real" ? activeClass : inactiveClass;
    nominalBtn.className = mode === "nominal" ? activeClass : inactiveClass;
  }

  if (retirementChart && (window as any)._retirementChartState) {
    const s = (window as any)._retirementChartState;
    const [p10, p25, p50, p75, p90, det] =
      mode === "real"
        ? [s.real.p10, s.real.p25, s.real.p50, s.real.p75, s.real.p90, s.real.deterministic]
        : [s.nom.p10, s.nom.p25, s.nom.p50, s.nom.p75, s.nom.p90, s.nom.deterministic];

    retirementChart.setOption(
      buildOption(p10, p25, p50, p75, p90, det, s.years, s.markLines, s.currency, s.locale)
    );
  }
};

// Called by retirement-simulate.ts after slider-driven API responses.
(window as any).updateRetirementChart = function (data: RetirementChartData) {
  const s = (window as any)._retirementChartState;
  if (!retirementChart || !s) return;

  const years = data.years.map(String);
  const markLines = buildMarkLines(data);
  const nom = {
    p10: toNominal(data.p10, s.inflation),
    p25: toNominal(data.p25, s.inflation),
    p50: toNominal(data.p50, s.inflation),
    p75: toNominal(data.p75, s.inflation),
    p90: toNominal(data.p90, s.inflation),
    deterministic: toNominal(data.deterministic, s.inflation),
  };

  s.real = {
    p10: data.p10, p25: data.p25, p50: data.p50,
    p75: data.p75, p90: data.p90, deterministic: data.deterministic,
  };
  s.nom = nom;
  s.years = years;
  s.markLines = markLines;

  const [p10, p25, p50, p75, p90, det] =
    currentMode === "real"
      ? [s.real.p10, s.real.p25, s.real.p50, s.real.p75, s.real.p90, s.real.deterministic]
      : [s.nom.p10, s.nom.p25, s.nom.p50, s.nom.p75, s.nom.p90, s.nom.deterministic];

  retirementChart.setOption(
    buildOption(p10, p25, p50, p75, p90, det, s.years, s.markLines, s.currency, s.locale)
  );
};

function initChart(container: HTMLElement): void {
  const scenarioId = container.dataset.scenarioId;
  const currency = container.dataset.currency ?? "EUR";
  const locale = container.dataset.locale ?? "de-DE";
  const inflation = parseFloat(container.dataset.inflation ?? "0.02");
  if (!scenarioId) return;

  fetch(`/api/retirement/${scenarioId}/chart`)
    .then((r) => {
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      return r.json() as Promise<RetirementChartData>;
    })
    .then((data) => {
      const years = data.years.map(String);
      const markLines = buildMarkLines(data);

      // Pre-compute nominal versions of every series.
      const nom = {
        p10: toNominal(data.p10, inflation),
        p25: toNominal(data.p25, inflation),
        p50: toNominal(data.p50, inflation),
        p75: toNominal(data.p75, inflation),
        p90: toNominal(data.p90, inflation),
        deterministic: toNominal(data.deterministic, inflation),
      };

      // Store state for mode switching and external updates.
      (window as any)._retirementChartState = {
        real: {
          p10: data.p10, p25: data.p25, p50: data.p50,
          p75: data.p75, p90: data.p90, deterministic: data.deterministic,
        },
        nom,
        years,
        markLines,
        currency,
        locale,
        inflation,
      };

      retirementChart = echarts.init(
        container,
        getTheme()
      );
      retirementChart.setOption(
        buildOption(
          data.p10, data.p25, data.p50, data.p75, data.p90,
          data.deterministic, years, markLines, currency, locale
        )
      );

      window.addEventListener("resize", () => retirementChart?.resize());

      document.addEventListener("theme-change", () => {
        retirementChart?.dispose();
        retirementChart = echarts.init(
          container,
          getTheme()
        );
        const s = (window as any)._retirementChartState;
        const [p10, p25, p50, p75, p90, det] =
          currentMode === "real"
            ? [s.real.p10, s.real.p25, s.real.p50, s.real.p75, s.real.p90, s.real.deterministic]
            : [s.nom.p10, s.nom.p25, s.nom.p50, s.nom.p75, s.nom.p90, s.nom.deterministic];
        retirementChart.setOption(
          buildOption(p10, p25, p50, p75, p90, det, s.years, s.markLines, s.currency, s.locale)
        );
      });
    })
    .catch((err) => {
      console.error("Retirement chart error:", err);
    });
}

document.addEventListener("DOMContentLoaded", () => {
  const container = document.getElementById("retirement-chart");
  if (container) initChart(container as HTMLElement);
});
