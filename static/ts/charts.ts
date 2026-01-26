declare const echarts: any;

interface CategoryData {
  category: string;
  color: string;
  amount_cents: number;
  percentage: number;
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

let categoryChart: any = null;
let timeChart: any = null;
let monthlyChart: any = null;

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

async function updateCategoryChart(params: URLSearchParams): Promise<void> {
  const container = document.getElementById("category-chart");
  if (!container) return;

  const data = await fetchData<CategoryData[]>(
    "/api/analytics/spending-by-category",
    params
  );

  if (categoryChart) {
    categoryChart.dispose();
  }

  categoryChart = echarts.init(container, getTheme());

  const option = {
    tooltip: {
      trigger: "item",
      formatter: (params: any) => {
        const value = params.value;
        const percentage = params.percent.toFixed(1);
        return `${params.name}: ${formatCurrency(value * 100)} (${percentage}%)`;
      },
    },
    legend: {
      orient: "horizontal",
      bottom: 0,
    },
    series: [
      {
        type: "pie",
        radius: ["40%", "70%"],
        avoidLabelOverlap: false,
        label: {
          show: false,
        },
        labelLine: {
          show: false,
        },
        data: data.map((d) => ({
          value: d.amount_cents / 100,
          name: d.category,
          itemStyle: { color: d.color },
        })),
      },
    ],
  };

  categoryChart.setOption(option);
}

async function updateTimeChart(params: URLSearchParams): Promise<void> {
  const container = document.getElementById("time-chart");
  if (!container) return;

  const data = await fetchData<TimeSeriesData[]>(
    "/api/analytics/spending-over-time",
    params
  );

  if (timeChart) {
    timeChart.dispose();
  }

  timeChart = echarts.init(container, getTheme());

  const option = {
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

  timeChart.setOption(option);
}

async function updateMonthlyChart(params: URLSearchParams): Promise<void> {
  const container = document.getElementById("monthly-chart");
  if (!container) return;

  const data = await fetchData<MonthlySummary[]>(
    "/api/analytics/monthly-summary",
    params
  );

  if (monthlyChart) {
    monthlyChart.dispose();
  }

  monthlyChart = echarts.init(container, getTheme());

  const option = {
    tooltip: {
      trigger: "axis",
      axisPointer: {
        type: "shadow",
      },
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

  monthlyChart.setOption(option);
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

  try {
    await Promise.all([
      updateCategoryChart(params),
      updateTimeChart(params),
      updateMonthlyChart(params),
    ]);
  } catch (error) {
    console.error("Failed to update charts:", error);
  }
}

function handleResize(): void {
  if (categoryChart) categoryChart.resize();
  if (timeChart) timeChart.resize();
  if (monthlyChart) monthlyChart.resize();
}

document.addEventListener("DOMContentLoaded", () => {
  if (document.getElementById("category-chart")) {
    updateCharts();
    window.addEventListener("resize", handleResize);
  }
});
