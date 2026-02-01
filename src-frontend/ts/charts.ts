declare const echarts: any;

interface CategoryTreeNode {
  name: string;
  color: string;
  id?: number;
  amount_cents?: number;
  children: CategoryTreeNode[];
}

interface CategoryTreeResponse {
  categories: CategoryTreeNode[];
  from_date?: string;
  to_date?: string;
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

interface SankeyData {
  nodes: { name: string; color?: string; depth: number }[];
  links: { source: string; target: string; value: number }[];
  from_date?: string;
  to_date?: string;
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
let activeMonth: string | null = null;
let activeCategory: number | null = null;

function showEmptyState(container: HTMLElement): void {
  if (activeChart) {
    activeChart.dispose();
    activeChart = null;
  }
  container.innerHTML =
    '<div class="flex items-center justify-center h-full min-h-[200px] text-neutral-400 dark:text-neutral-500 text-sm">' +
    "No data for the selected period" +
    "</div>";
}

function formatCurrency(cents: number): string {
  return "$" + (cents / 100).toFixed(2);
}

function isDarkMode(): boolean {
  return document.documentElement.classList.contains("dark");
}

function getTheme(): string | undefined {
  return isDarkMode() ? "dark" : undefined;
}

async function fetchData<T>(
  endpoint: string,
  params: URLSearchParams,
): Promise<T> {
  const response = await fetch(`${endpoint}?${params.toString()}`);
  if (!response.ok) throw new Error("Failed to fetch data");
  return response.json();
}

function mapTreeToSunburst(nodes: CategoryTreeNode[]): any[] {
  return nodes.map((node) => {
    if (node.children.length > 0) {
      return {
        name: node.name,
        categoryId: node.id,
        itemStyle: { color: node.color },
        children: mapTreeToSunburst(node.children),
      };
    }
    return {
      name: node.name,
      categoryId: node.id,
      value: (node.amount_cents || 0) / 100,
      itemStyle: { color: node.color },
    };
  });
}

function treeMaxDepth(nodes: CategoryTreeNode[]): number {
  let max = 0;
  for (const node of nodes) {
    if (node.children.length > 0) {
      max = Math.max(max, 1 + treeMaxDepth(node.children));
    }
  }
  return max;
}

function buildSunburstLevels(maxDepth: number): any[] {
  const levels: any[] = [{}];
  for (let i = 1; i <= maxDepth; i++) {
    const r0Pct = 10 + ((i - 1) / maxDepth) * 80;
    const rPct = 10 + (i / maxDepth) * 80;
    levels.push({
      r0: `${r0Pct}%`,
      r: `${rPct}%`,
      itemStyle: { opacity: Math.max(0.5, 1 - (i - 1) * 0.15) },
      label: {
        rotate: i < maxDepth ? "radial" : undefined,
        align: i === maxDepth ? "right" : undefined,
        fontSize: Math.max(8, 13 - i),
      },
    });
  }
  return levels;
}

function collapseCategoryTransactions(): void {
  const container = document.getElementById("category-transactions");
  if (!container) return;
  container.style.maxHeight = "0";
  container.style.opacity = "0";
  activeCategory = null;
}

function setupCategoryShiftClick(
  fromDate?: string,
  toDate?: string,
): void {
  if (!activeChart) return;

  activeChart.on("click", (params: any) => {
    if (!params.event?.event?.shiftKey) return;

    const categoryId: number | undefined = params.data?.categoryId;
    const hasChildren = params.data?.children?.length > 0;
    const isUncategorized = params.name === "Uncategorized" && categoryId == null;

    const container = document.getElementById("category-transactions");
    if (!container) return;

    // Build a key for toggle detection
    const clickKey = isUncategorized ? -1 : (categoryId ?? -2);

    // Clicking the same category again collapses the panel
    if (clickKey === activeCategory) {
      collapseCategoryTransactions();
      return;
    }

    const url = new URL(
      "/spending/category-transactions",
      window.location.origin,
    );

    if (isUncategorized) {
      url.searchParams.set("uncategorized", "true");
    } else if (categoryId != null) {
      url.searchParams.set("category_id", String(categoryId));
      url.searchParams.set("include_children", String(hasChildren));
    } else {
      return;
    }

    if (fromDate) url.searchParams.set("from_date", fromDate);
    if (toDate) url.searchParams.set("to_date", toDate);

    // If replacing content, snap to 0 first to re-trigger transition
    if (activeCategory !== null) {
      container.style.transition = "none";
      container.style.maxHeight = "0";
      container.style.opacity = "0";
      void container.offsetHeight;
      container.style.transition = "";
    }

    fetch(url.toString())
      .then((res) => {
        if (!res.ok) throw new Error("Failed to fetch transactions");
        return res.text();
      })
      .then((html) => {
        container.innerHTML = html;
        activeCategory = clickKey;

        requestAnimationFrame(() => {
          container.style.maxHeight = container.scrollHeight + "px";
          container.style.opacity = "1";

          const closeBtn = container.querySelector(".preview-close-btn");
          if (closeBtn) {
            closeBtn.addEventListener(
              "click",
              collapseCategoryTransactions,
            );
          }
        });

        const savedKey = clickKey;
        const onTransitionEnd = () => {
          if (activeCategory === savedKey) {
            container.style.maxHeight = "none";
          }
          container.removeEventListener("transitionend", onTransitionEnd);
        };
        container.addEventListener("transitionend", onTransitionEnd);
      })
      .catch((err) =>
        console.error("Category transactions fetch error:", err),
      );
  });
}

async function updateCategoryChart(params: URLSearchParams): Promise<void> {
  const container = document.getElementById("category-chart");
  if (!container) return;

  collapseCategoryTransactions();

  const data = await fetchData<CategoryTreeResponse>(
    "/api/analytics/spending-by-category-tree",
    params,
  );

  if (data.categories.length === 0) {
    showEmptyState(container);
    return;
  }

  if (activeChart) {
    activeChart.dispose();
  }

  activeChart = echarts.init(container, getTheme());

  const dark = isDarkMode();
  const borderColor = dark ? "#262626" : "#ffffff";
  const months = getMonthSpan(data.from_date, data.to_date);

  const option = {
    backgroundColor: "transparent",
    tooltip: {
      trigger: "item",
      formatter: (params: any) => {
        const value = params.value;
        if (value == null) {
          return `<strong>${params.name}</strong>`;
        }
        const amount = formatCurrency(value * 100);
        const perMonth = formatCurrency((value * 100) / months);
        return (
          `${params.name}: ${amount}` +
          `<br/><span style="font-size:0.85em;opacity:0.7">${perMonth}/mo</span>`
        );
      },
    },
    series: [
      {
        type: "sunburst",
        radius: ["0%", "90%"],
        data: mapTreeToSunburst(data.categories),
        sort: "desc",
        itemStyle: {
          borderRadius: 4,
          borderWidth: 2,
          borderColor: borderColor,
        },
        levels: buildSunburstLevels(Math.max(1, treeMaxDepth(data.categories))),
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
  setupCategoryShiftClick(data.from_date, data.to_date);
}

async function updateTimeChart(params: URLSearchParams): Promise<void> {
  const container = document.getElementById("time-chart");
  if (!container) return;

  const data = await fetchData<TimeSeriesData[]>(
    "/api/analytics/spending-over-time",
    params,
  );

  if (data.length === 0) {
    showEmptyState(container);
    return;
  }

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
    ".category-checkbox:checked",
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

  if (selectedIds.length > 0) {
    // Multi-bar mode: one series per selected category
    const catParams = new URLSearchParams(params);
    catParams.set("category_ids", selectedIds.join(","));

    const data = await fetchData<MonthlyByCategoryResponse>(
      "/api/analytics/monthly-by-category",
      catParams,
    );

    if (data.series.length === 0) {
      showEmptyState(container);
      return;
    }

    if (activeChart) {
      activeChart.dispose();
    }
    activeChart = echarts.init(container, getTheme());

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
    collapseMonthlyTransactions();
    setupMonthlyBarClick();
  } else {
    // Aggregate mode: single bar series
    const data = await fetchData<MonthlySummary[]>(
      "/api/analytics/monthly-summary",
      params,
    );

    if (data.length === 0) {
      showEmptyState(container);
      return;
    }

    if (activeChart) {
      activeChart.dispose();
    }
    activeChart = echarts.init(container, getTheme());

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
    collapseMonthlyTransactions();
    setupMonthlyBarClick();
  }
}

function collapseMonthlyTransactions(): void {
  const container = document.getElementById("monthly-transactions");
  if (!container) return;
  container.style.maxHeight = "0";
  container.style.opacity = "0";
  activeMonth = null;
}

function setupMonthlyBarClick(): void {
  if (!activeChart) return;

  activeChart.on("click", (params: any) => {
    const month = params.name;
    if (!month) return;

    const container = document.getElementById("monthly-transactions");
    if (!container) return;

    // Clicking the same month again collapses the panel
    if (month === activeMonth) {
      collapseMonthlyTransactions();
      return;
    }

    const selectedIds = getSelectedCategoryIds();
    const url = new URL(
      "/spending/monthly-transactions",
      window.location.origin,
    );
    url.searchParams.set("month", month);
    if (selectedIds.length > 0) {
      url.searchParams.set("category_ids", selectedIds.join(","));
    }

    // If replacing content, snap to 0 first to re-trigger transition
    if (activeMonth) {
      container.style.transition = "none";
      container.style.maxHeight = "0";
      container.style.opacity = "0";
      // Force reflow
      void container.offsetHeight;
      container.style.transition = "";
    }

    fetch(url.toString())
      .then((res) => {
        if (!res.ok) throw new Error("Failed to fetch transactions");
        return res.text();
      })
      .then((html) => {
        container.innerHTML = html;
        activeMonth = month;

        // Animate in
        requestAnimationFrame(() => {
          container.style.maxHeight = container.scrollHeight + "px";
          container.style.opacity = "1";

          // Wire up close button
          const closeBtn = container.querySelector(".preview-close-btn");
          if (closeBtn) {
            closeBtn.addEventListener("click", collapseMonthlyTransactions);
          }
        });

        // After transition, remove max-height cap so content isn't clipped
        const onTransitionEnd = () => {
          if (activeMonth === month) {
            container.style.maxHeight = "none";
          }
          container.removeEventListener("transitionend", onTransitionEnd);
        };
        container.addEventListener("transitionend", onTransitionEnd);
      })
      .catch((err) => console.error("Monthly transactions fetch error:", err));
  });
}

async function updateFlowChart(params: URLSearchParams): Promise<void> {
  const container = document.getElementById("flow-chart");
  if (!container) return;

  const data = await fetchData<SankeyData>(
    "/api/analytics/flow-sankey",
    params,
  );

  if (data.nodes.length === 0) {
    showEmptyState(container);
    return;
  }

  if (activeChart) {
    activeChart.dispose();
  }

  activeChart = echarts.init(container, getTheme(), { renderer: "canvas" });

  const dark = isDarkMode();
  const compact = container.clientWidth < 500;

  const option = {
    backgroundColor: "transparent",
    tooltip: {
      trigger: "item",
      formatter: (params: any) => {
        if (params.dataType === "edge") {
          const amount = formatCurrency(params.data.value * 100);
          const months = getMonthSpan(data.from_date, data.to_date);
          const perMonth = formatCurrency((params.data.value * 100) / months);
          return (
            `${params.data.source} → ${params.data.target}: ${amount}` +
            `<br/><span style="font-size:0.85em;opacity:0.7">${perMonth}/mo</span>`
          );
        }
        return `<strong>${params.name}</strong>`;
      },
    },
    series: [
      {
        type: "sankey",
        top: 20,
        bottom: 20,
        left: compact ? 10 : 20,
        right: compact ? 80 : 120,
        emphasis: {
          focus: "adjacency",
        },
        nodeAlign: "justify",
        nodeGap: 12,
        nodeWidth: 20,
        layoutIterations: 0,
        sort: null,
        data: data.nodes.map((n) => ({
          name: n.name,
          depth: n.depth,
          itemStyle: n.color ? { color: n.color } : {},
        })),
        links: data.links,
        lineStyle: {
          color: "gradient",
          opacity: 0.4,
        },
        label: {
          color: dark ? "#e5e5e5" : "#262626",
          fontSize: 12,
        },
      },
    ],
  };

  activeChart.setOption(option);
  activeChart.resize();
}

function getMonthSpan(fromDate?: string, toDate?: string): number {
  const fromStr =
    fromDate ||
    (document.getElementById("from_date") as HTMLInputElement)?.value;
  const toStr =
    toDate || (document.getElementById("to_date") as HTMLInputElement)?.value;
  if (!fromStr || !toStr) return 1;
  const from = new Date(fromStr + "T00:00:00");
  const to = new Date(toStr + "T00:00:00");
  const days = (to.getTime() - from.getTime()) / (1000 * 60 * 60 * 24) + 1;
  return Math.max(1, days / (365.25 / 12));
}

function getFilterParams(): URLSearchParams {
  const params = new URLSearchParams();
  const fromDate = (document.getElementById("from_date") as HTMLInputElement)
    ?.value;
  const toDate = (document.getElementById("to_date") as HTMLInputElement)
    ?.value;

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
    } else if (activeTab === "flow") {
      await updateFlowChart(params);
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
    const boxes =
      dropdown!.querySelectorAll<HTMLInputElement>(".category-checkbox");
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
  const initialIds = new URLSearchParams(window.location.search).get(
    "categories",
  );
  if (initialIds) {
    const idSet = new Set(initialIds.split(","));
    const boxes =
      dropdown.querySelectorAll<HTMLInputElement>(".category-checkbox");
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
        ".category-checkbox:checked",
      );
      const name = checked
        ?.closest("label")
        ?.querySelector("span:last-child")
        ?.textContent?.trim();
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
        ".category-checkbox:checked",
      );
      const name = checked
        ?.closest("label")
        ?.querySelector("span:last-child")
        ?.textContent?.trim();
      label.textContent = name || "1 selected";
    } else if (selected.length > 1) {
      label.textContent = `${selected.length} selected`;
    }
  }
}

document.addEventListener("DOMContentLoaded", () => {
  if (document.querySelector("[data-active-tab]")) {
    updateNavLinks();
    setupCategoryFilter();
    updateCharts();
    window.addEventListener("resize", handleResize);
  }
});
