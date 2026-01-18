declare const Chart: any;

interface NetWorthChartResponse {
  labels: string[];
  net_worth: number[];
  expense_component: number[];
  portfolio_component: number[];
}

let netWorthChart: any = null;

function formatMoney(cents: number): string {
  const dollars = cents / 100;
  return (
    "$" +
    dollars.toLocaleString("en-US", {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    })
  );
}

function getChartColors(): {
  text: string;
  grid: string;
  netWorth: string;
  netWorthFill: string;
  expense: string;
  portfolio: string;
} {
  const isDark = document.documentElement.classList.contains("dark");
  return {
    text: isDark ? "#e5e7eb" : "#374151",
    grid: isDark ? "#374151" : "#e5e7eb",
    netWorth: "#3b82f6", // blue
    netWorthFill: isDark ? "rgba(59, 130, 246, 0.15)" : "rgba(59, 130, 246, 0.1)",
    expense: "#22c55e", // green
    portfolio: "#f59e0b", // amber
  };
}

async function loadNetWorthChart(): Promise<void> {
  const canvas = document.getElementById("net-worth-chart") as HTMLCanvasElement;
  if (!canvas) return;

  try {
    const response = await fetch("/api/net-worth/chart");
    if (!response.ok) throw new Error("Failed to fetch data");

    const data: NetWorthChartResponse = await response.json();
    const colors = getChartColors();

    if (netWorthChart) {
      netWorthChart.destroy();
    }

    // Convert cents to dollars for display
    const netWorthDollars = data.net_worth.map((c) => c / 100);
    const expenseDollars = data.expense_component.map((c) => c / 100);
    const portfolioDollars = data.portfolio_component.map((c) => c / 100);

    netWorthChart = new Chart(canvas, {
      type: "line",
      data: {
        labels: data.labels,
        datasets: [
          {
            label: "Net Worth",
            data: netWorthDollars,
            borderColor: colors.netWorth,
            backgroundColor: colors.netWorthFill,
            fill: true,
            tension: 0.1,
            pointRadius: data.labels.length > 100 ? 0 : 2,
            pointHoverRadius: 4,
            borderWidth: 2,
            order: 1,
          },
          {
            label: "Expenses (Cumulative)",
            data: expenseDollars,
            borderColor: colors.expense,
            backgroundColor: "transparent",
            borderDash: [5, 5],
            fill: false,
            tension: 0.1,
            pointRadius: 0,
            pointHoverRadius: 4,
            borderWidth: 1.5,
            hidden: true,
            order: 2,
          },
          {
            label: "Portfolio Value",
            data: portfolioDollars,
            borderColor: colors.portfolio,
            backgroundColor: "transparent",
            borderDash: [5, 5],
            fill: false,
            tension: 0.1,
            pointRadius: 0,
            pointHoverRadius: 4,
            borderWidth: 1.5,
            hidden: true,
            order: 3,
          },
        ],
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        interaction: {
          intersect: false,
          mode: "index",
        },
        plugins: {
          legend: {
            display: true,
            position: "top",
            labels: {
              color: colors.text,
              usePointStyle: true,
              padding: 16,
            },
          },
          tooltip: {
            callbacks: {
              label: (context: any) => {
                const label = context.dataset.label || "";
                const value = formatMoney(context.raw * 100);
                return `${label}: ${value}`;
              },
            },
          },
        },
        scales: {
          x: {
            type: "category",
            grid: { color: colors.grid },
            ticks: {
              color: colors.text,
              maxTicksLimit: 12,
              maxRotation: 45,
            },
          },
          y: {
            grid: { color: colors.grid },
            ticks: {
              color: colors.text,
              callback: (value: number) => formatMoney(value * 100),
            },
          },
        },
      },
    });
  } catch (error) {
    console.error("Failed to load net worth chart:", error);
    const container = canvas.parentElement;
    if (container) {
      container.innerHTML = `
        <div class="flex items-center justify-center h-full text-neutral-500">
          Failed to load chart data.
        </div>
      `;
    }
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const chartElement = document.getElementById("net-worth-chart");
  if (chartElement) {
    loadNetWorthChart();
  }
});
