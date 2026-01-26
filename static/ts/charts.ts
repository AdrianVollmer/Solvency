declare const echarts: any;

interface CategoryTreeNode {
  name: string;
  color: string;
  amount_cents?: number;
  children: CategoryTreeNode[];
}

interface TimeSeriesData {
  date: string;
  amount_cents: number;
}

interface MonthlySummary {
  month: string;
  total_cents: number;
  transaction_count: number;
  average_cents: number;
}

interface MonthlyByCategoryResponse {
  months: string[];
  series: {
    category: string;
    color: string;
    totals: number[];
  }[];
}

let activeChart: any = null;

function formatCurrency(cents: number): string {
  return "$" + (cents / 100).toFixed(2);
}

function isDarkMode(): boolean {
  return document.documentElement.classList.contains("dark");
}

function getTheme(): string | undefined {
  return isDarkMode() ? "dark" : undefined;
}

async function fetchData<T>(endpoint: string, params: URLSearchParams): Promise<T> {
  const response = await fetch(`${endpoint}?${params.toString()}`);
  if (!response.ok) throw new Error("Failed to fetch data");
  return response.json();
}

function mapTreeToSunburst(nodes: CategoryTreeNode[]): any[] {
  return nodes.map((node) => {
    if (node.children.length > 0) {
      return {
        name: node.name,
        itemStyle: { color: node.color },
        children: mapTreeToSunburst(node.children),
      };
    }
    return {
      name: node.name,
      value: (node.amount_cents || 0) / 100,
      itemStyle: { color: node.color },
    };
  });
}

async function updateCategoryChart(params: URLSearchParams): Promise<void> {
  const container = document.getElementById("category-chart");
  if (!container) return;

  const data = await fetchData<CategoryTreeNode[]>(
    "/api/analytics/spending-by-category-tree",
    params
  );

  if (activeChart) {
    activeChart.dispose();
  }

  activeChart = echarts.init(container, getTheme());

  const dark = isDarkMode();
  const borderColor = dark ? "#262626" : "#ffffff";

  const option = {
    backgroundColor: "transparent",
    tooltip: {
      trigger: "item",
      formatter: (params: any) => {
        const value = params.value;
        if (value == null) {
          return `<strong>${params.name}</strong>`;
        }
        return `${params.name}: ${formatCurrency(value * 100)}`;
      },
    },
    series: [
      {
        type: "sunburst",
        radius: ["0%", "90%"],
        data: mapTreeToSunburst(data),
        sort: "desc",
        itemStyle: {
          borderRadius: 4,
          borderWidth: 2,
          borderColor: borderColor,
        },
        levels: [
          {},
          {
            r0: "10%",
            r: "50%",
            itemStyle: {
              opacity: 1,
            },
            label: {
              rotate: "tangential",
              fontSize: 12,
            },
          },
          {
            r0: "50%",
            r: "90%",
            itemStyle: {
              opacity: 0.75,
            },
            label: {
              align: "right",
              fontSize: 10,
            },
          },
        ],
        label: {
          show: true,
          color: dark ? "#e5e5e5" : "#262626",
        },
        emphasis: {
          focus: "ancestor",
        },
      },
    ],
  };

  activeChart.setOption(option);
}

async function updateTimeChart(params: URLSearchParams): Promise<void> {
  const container = document.getElementById("time-chart");
  if (!container) return;

  const data = await fetchData<TimeSeriesData[]>(
    "/api/analytics/spending-over-time",
    params
  );

  if (activeChart) {
    activeChart.dispose();
  }

  activeChart = echarts.init(container, getTheme());

  const option = {
    backgroundColor: "transparent",
    tooltip: {
      trigger: "axis",
      formatter: (params: any) => {
        const point = params[0];
        return `${point.axisValue}<br/>${formatCurrency(point.value * 100)}`;
      },
    },
    grid: {
      left: "3%",
      right: "4%",
      bottom: "3%",
      containLabel: true,
    },
    xAxis: {
      type: "category",
      boundaryGap: false,
      data: data.map((d) => d.date),
    },
    yAxis: {
      type: "value",
      axisLabel: {
        formatter: (value: number) => formatCurrency(value * 100),
      },
    },
    series: [
      {
        name: "Daily Spending",
        type: "line",
        smooth: true,
        areaStyle: {
          opacity: 0.1,
        },
        lineStyle: {
          color: "#22c55e",
        },
        itemStyle: {
          color: "#22c55e",
        },
        data: data.map((d) => d.amount_cents / 100),
      },
    ],
  };

  activeChart.setOption(option);
}

function getSelectedCategoryIds(): string[] {
  const checkboxes = document.querySelectorAll<HTMLInputElement>(
    ".category-checkbox:checked"
  );
  const ids: string[] = [];
  for (const cb of checkboxes) {
    ids.push(cb.value);
  }
  return ids;
}

async function updateMonthlyChart(params: URLSearchParams): Promise<void> {
  const container = document.getElementById("monthly-chart");
  if (!container) return;

  const selectedIds = getSelectedCategoryIds();

  if (activeChart) {
    activeChart.dispose();
  }

  activeChart = echarts.init(container, getTheme());

  if (selectedIds.length > 0) {
    // Multi-bar mode: one series per selected category
    const catParams = new URLSearchParams(params);
    catParams.set("category_ids", selectedIds.join(","));

    const data = await fetchData<MonthlyByCategoryResponse>(
      "/api/analytics/monthly-by-category",
      catParams
    );

    const series = data.series.map((s) => ({
      name: s.category,
      type: "bar",
      itemStyle: {
        color: s.color,
        borderRadius: [4, 4, 0, 0],
      },
      data: s.totals.map((v) => v / 100),
    }));

    const option = {
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "shadow" },
        formatter: (params: any) => {
          let html = `<strong>${params[0].axisValue}</strong>`;
          for (const p of params) {
            html += `<br/>${p.marker} ${p.seriesName}: ${formatCurrency(p.value * 100)}`;
          }
          return html;
        },
      },
      legend: {
        bottom: 0,
      },
      grid: {
        left: "3%",
        right: "4%",
        bottom: "10%",
        containLabel: true,
      },
      xAxis: {
        type: "category",
        data: data.months,
      },
      yAxis: {
        type: "value",
        axisLabel: {
          formatter: (value: number) => formatCurrency(value * 100),
        },
      },
      series: series,
    };

    activeChart.setOption(option);
  } else {
    // Aggregate mode: single bar series
    const data = await fetchData<MonthlySummary[]>(
      "/api/analytics/monthly-summary",
      params
    );

    const option = {
      backgroundColor: "transparent",
      tooltip: {
        trigger: "axis",
        axisPointer: { type: "shadow" },
        formatter: (params: any) => {
          const point = params[0];
          const item = data[point.dataIndex];
          return [
            `<strong>${point.axisValue}</strong>`,
            `Total: ${formatCurrency(item.total_cents)}`,
            `Transactions: ${item.transaction_count}`,
            `Average: ${formatCurrency(item.average_cents)}`,
          ].join("<br/>");
        },
      },
      grid: {
        left: "3%",
        right: "4%",
        bottom: "3%",
        containLabel: true,
      },
      xAxis: {
        type: "category",
        data: data.map((d) => d.month),
      },
      yAxis: {
        type: "value",
        axisLabel: {
          formatter: (value: number) => formatCurrency(value * 100),
        },
      },
      series: [
        {
          name: "Monthly Total",
          type: "bar",
          barWidth: "60%",
          itemStyle: {
            color: "#3b82f6",
            borderRadius: [4, 4, 0, 0],
          },
          data: data.map((d) => d.total_cents / 100),
        },
      ],
    };

    activeChart.setOption(option);
  }
}

function getFilterParams(): URLSearchParams {
  const params = new URLSearchParams();
  const fromDate = (document.getElementById("from_date") as HTMLInputElement)?.value;
  const toDate = (document.getElementById("to_date") as HTMLInputElement)?.value;

  if (fromDate) params.set("from_date", fromDate);
  if (toDate) params.set("to_date", toDate);

  return params;
}

async function updateCharts(): Promise<void> {
  const params = getFilterParams();

  const tabEl = document.querySelector("[data-active-tab]");
  const activeTab = tabEl?.getAttribute("data-active-tab") || "category";

  try {
    if (activeTab === "category") {
      await updateCategoryChart(params);
    } else if (activeTab === "time") {
      await updateTimeChart(params);
    } else if (activeTab === "monthly") {
      await updateMonthlyChart(params);
    }
  } catch (error) {
    console.error("Failed to update chart:", error);
  }
}

function handleResize(): void {
  if (activeChart) activeChart.resize();
}

// Rewrite [data-nav] links so every href carries the full current state.
// Each link declares only the params it *changes*; this function merges
// those overrides with the current URL, resolving preset/date conflicts.
function updateNavLinks(): void {
  const current = new URLSearchParams(window.location.search);
  const links = document.querySelectorAll<HTMLAnchorElement>("[data-nav]");

  for (const link of links) {
    const overrides = new URLSearchParams(link.dataset.nav || "");
    const merged = new URLSearchParams(current);

    // Preset and explicit dates are mutually exclusive
    if (overrides.has("preset")) {
      merged.delete("from_date");
      merged.delete("to_date");
    }
    if (overrides.has("from_date") || overrides.has("to_date")) {
      merged.delete("preset");
    }

    for (const [key, value] of overrides) {
      merged.set(key, value);
    }

    link.href = `/spending?${merged.toString()}`;
  }
}

// Category filter dropdown logic
function setupCategoryFilter(): void {
  const btn = document.getElementById("category-filter-btn");
  const dropdown = document.getElementById("category-filter-dropdown");
  const label = document.getElementById("category-filter-label");

  if (!btn || !dropdown || !label) return;

  // Toggle dropdown
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    dropdown.classList.toggle("hidden");
  });

  // Close on outside click
  document.addEventListener("click", (e) => {
    if (!dropdown.contains(e.target as Node) && e.target !== btn) {
      dropdown.classList.add("hidden");
    }
  });

  // Select all / clear buttons
  const selectAllBtn = document.getElementById("category-select-all");
  const selectNoneBtn = document.getElementById("category-select-none");

  function setAllCheckboxes(checked: boolean): void {
    const boxes = dropdown!.querySelectorAll<HTMLInputElement>(".category-checkbox");
    for (const cb of boxes) {
      cb.checked = checked;
    }
    dropdown!.dispatchEvent(new Event("change"));
  }

  if (selectAllBtn) {
    selectAllBtn.addEventListener("click", () => setAllCheckboxes(true));
  }
  if (selectNoneBtn) {
    selectNoneBtn.addEventListener("click", () => setAllCheckboxes(false));
  }

  // Restore checkbox state from URL
  const initialIds = new URLSearchParams(window.location.search).get("categories");
  if (initialIds) {
    const idSet = new Set(initialIds.split(","));
    const boxes = dropdown.querySelectorAll<HTMLInputElement>(".category-checkbox");
    for (const cb of boxes) {
      cb.checked = idSet.has(cb.value);
    }
  }

  // On checkbox change: update label, sync URL, re-fetch chart
  function syncCategoryState(): void {
    const selected = getSelectedCategoryIds();

    // Update button label
    if (selected.length === 0) {
      label!.textContent = "All categories";
    } else if (selected.length === 1) {
      const checked = dropdown!.querySelector<HTMLInputElement>(
        ".category-checkbox:checked"
      );
      const name = checked
        ?.closest("label")
        ?.querySelector("span:last-child")?.textContent?.trim();
      label!.textContent = name || "1 selected";
    } else {
      label!.textContent = `${selected.length} selected`;
    }

    // Push to URL so navigation links and reloads preserve the selection
    const url = new URL(window.location.href);
    if (selected.length > 0) {
      url.searchParams.set("categories", selected.join(","));
    } else {
      url.searchParams.delete("categories");
    }
    history.replaceState(null, "", url.toString());

    updateNavLinks();
    updateMonthlyChart(getFilterParams());
  }

  dropdown.addEventListener("change", syncCategoryState);

  // If we restored checkboxes from URL, update the label (but don't re-fetch,
  // updateCharts() handles the initial load)
  if (initialIds) {
    const selected = getSelectedCategoryIds();
    if (selected.length === 1) {
      const checked = dropdown.querySelector<HTMLInputElement>(
        ".category-checkbox:checked"
      );
      const name = checked
        ?.closest("label")
        ?.querySelector("span:last-child")?.textContent?.trim();
      label.textContent = name || "1 selected";
    } else if (selected.length > 1) {
      label.textContent = `${selected.length} selected`;
    }
  }
}

document.addEventListener("DOMContentLoaded", () => {
  if (document.querySelector("[data-active-tab]")) {
    updateNavLinks();
    updateCharts();
    setupCategoryFilter();
    window.addEventListener("resize", handleResize);
  }
});
