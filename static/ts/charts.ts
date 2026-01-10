declare const Chart: any;

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
  expense_count: number;
  average_cents: number;
}

let categoryChart: any = null;
let timeChart: any = null;
let monthlyChart: any = null;

function formatCurrency(cents: number): string {
  return "$" + (cents / 100).toFixed(2);
}

function getChartColors(): { text: string; grid: string; background: string } {
  const isDark = document.documentElement.classList.contains("dark");
  return {
    text: isDark ? "#e5e7eb" : "#374151",
    grid: isDark ? "#374151" : "#e5e7eb",
    background: isDark ? "#1f2937" : "#ffffff",
  };
}

async function fetchData<T>(endpoint: string, params: URLSearchParams): Promise<T> {
  const response = await fetch(`${endpoint}?${params.toString()}`);
  if (!response.ok) throw new Error("Failed to fetch data");
  return response.json();
}

async function updateCategoryChart(params: URLSearchParams): Promise<void> {
  const canvas = document.getElementById("category-chart") as HTMLCanvasElement;
  if (!canvas) return;

  const data = await fetchData<CategoryData[]>(
    "/api/analytics/spending-by-category",
    params
  );
  const colors = getChartColors();

  if (categoryChart) {
    categoryChart.destroy();
  }

  categoryChart = new Chart(canvas, {
    type: "doughnut",
    data: {
      labels: data.map((d) => d.category),
      datasets: [
        {
          data: data.map((d) => d.amount_cents / 100),
          backgroundColor: data.map((d) => d.color),
          borderWidth: 0,
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: true,
      plugins: {
        legend: {
          position: "bottom",
          labels: { color: colors.text },
        },
        tooltip: {
          callbacks: {
            label: (context: any) => {
              const value = context.raw;
              const percentage = data[context.dataIndex].percentage.toFixed(1);
              return `${formatCurrency(value * 100)} (${percentage}%)`;
            },
          },
        },
      },
    },
  });
}

async function updateTimeChart(params: URLSearchParams): Promise<void> {
  const canvas = document.getElementById("time-chart") as HTMLCanvasElement;
  if (!canvas) return;

  const data = await fetchData<TimeSeriesData[]>(
    "/api/analytics/spending-over-time",
    params
  );
  const colors = getChartColors();

  if (timeChart) {
    timeChart.destroy();
  }

  timeChart = new Chart(canvas, {
    type: "line",
    data: {
      labels: data.map((d) => d.date),
      datasets: [
        {
          label: "Daily Spending",
          data: data.map((d) => d.amount_cents / 100),
          borderColor: "#22c55e",
          backgroundColor: "rgba(34, 197, 94, 0.1)",
          fill: true,
          tension: 0.3,
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: true,
      plugins: {
        legend: {
          display: false,
        },
        tooltip: {
          callbacks: {
            label: (context: any) => formatCurrency(context.raw * 100),
          },
        },
      },
      scales: {
        x: {
          grid: { color: colors.grid },
          ticks: { color: colors.text },
        },
        y: {
          grid: { color: colors.grid },
          ticks: {
            color: colors.text,
            callback: (value: number) => formatCurrency(value * 100),
          },
        },
      },
    },
  });
}

async function updateMonthlyChart(params: URLSearchParams): Promise<void> {
  const canvas = document.getElementById("monthly-chart") as HTMLCanvasElement;
  if (!canvas) return;

  const data = await fetchData<MonthlySummary[]>(
    "/api/analytics/monthly-summary",
    params
  );
  const colors = getChartColors();

  if (monthlyChart) {
    monthlyChart.destroy();
  }

  monthlyChart = new Chart(canvas, {
    type: "bar",
    data: {
      labels: data.map((d) => d.month),
      datasets: [
        {
          label: "Monthly Total",
          data: data.map((d) => d.total_cents / 100),
          backgroundColor: "#3b82f6",
          borderRadius: 4,
        },
      ],
    },
    options: {
      responsive: true,
      maintainAspectRatio: true,
      plugins: {
        legend: {
          display: false,
        },
        tooltip: {
          callbacks: {
            label: (context: any) => {
              const item = data[context.dataIndex];
              return [
                `Total: ${formatCurrency(item.total_cents)}`,
                `Expenses: ${item.expense_count}`,
                `Average: ${formatCurrency(item.average_cents)}`,
              ];
            },
          },
        },
      },
      scales: {
        x: {
          grid: { display: false },
          ticks: { color: colors.text },
        },
        y: {
          grid: { color: colors.grid },
          ticks: {
            color: colors.text,
            callback: (value: number) => formatCurrency(value * 100),
          },
        },
      },
    },
  });
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

document.addEventListener("DOMContentLoaded", () => {
  if (document.getElementById("category-chart")) {
    const today = new Date();
    const threeMonthsAgo = new Date(today);
    threeMonthsAgo.setMonth(today.getMonth() - 3);

    const fromInput = document.getElementById("from_date") as HTMLInputElement;
    const toInput = document.getElementById("to_date") as HTMLInputElement;

    if (fromInput && !fromInput.value) {
      fromInput.value = threeMonthsAgo.toISOString().split("T")[0];
    }
    if (toInput && !toInput.value) {
      toInput.value = today.toISOString().split("T")[0];
    }

    updateCharts();
  }
});

(window as any).updateCharts = updateCharts;
