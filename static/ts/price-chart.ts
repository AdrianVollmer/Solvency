declare const Chart: any;

interface PriceData {
  date: string;
  price_cents: number;
}

interface ChartResponse {
  symbol: string;
  data: PriceData[];
  missing_ranges: [string, string][];
}

let priceChart: any = null;

function formatPrice(cents: number): string {
  return "$" + (cents / 100).toFixed(2);
}

function getChartColors(): { text: string; grid: string; line: string; fill: string } {
  const isDark = document.documentElement.classList.contains("dark");
  return {
    text: isDark ? "#e5e7eb" : "#374151",
    grid: isDark ? "#374151" : "#e5e7eb",
    line: "#3b82f6",
    fill: isDark ? "rgba(59, 130, 246, 0.1)" : "rgba(59, 130, 246, 0.1)",
  };
}

async function loadPriceChart(symbol: string): Promise<void> {
  const canvas = document.getElementById("price-chart") as HTMLCanvasElement;
  if (!canvas) return;

  try {
    const response = await fetch(`/api/market-data/${encodeURIComponent(symbol)}`);
    if (!response.ok) throw new Error("Failed to fetch data");

    const chartData: ChartResponse = await response.json();
    const colors = getChartColors();

    if (priceChart) {
      priceChart.destroy();
    }

    // Create annotations for missing ranges
    const annotations: any = {};
    for (const [i, [start, end]] of chartData.missing_ranges.entries()) {
      annotations[`missing${i}`] = {
        type: "box",
        xMin: start,
        xMax: end,
        backgroundColor: "rgba(239, 68, 68, 0.15)",
        borderColor: "rgba(239, 68, 68, 0.3)",
        borderWidth: 1,
        label: {
          display: true,
          content: "Missing",
          position: "center",
          color: "#ef4444",
          font: { size: 10 },
        },
      };
    }

    priceChart = new Chart(canvas, {
      type: "line",
      data: {
        labels: chartData.data.map((d) => d.date),
        datasets: [
          {
            label: `${symbol} Price`,
            data: chartData.data.map((d) => d.price_cents / 100),
            borderColor: colors.line,
            backgroundColor: colors.fill,
            fill: true,
            tension: 0.1,
            pointRadius: chartData.data.length > 100 ? 0 : 2,
            pointHoverRadius: 4,
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
            display: false,
          },
          tooltip: {
            callbacks: {
              label: (context: any) => formatPrice(context.raw * 100),
            },
          },
          annotation: {
            annotations: annotations,
          },
        },
        scales: {
          x: {
            type: "category",
            grid: { color: colors.grid },
            ticks: {
              color: colors.text,
              maxTicksLimit: 10,
              maxRotation: 45,
            },
          },
          y: {
            grid: { color: colors.grid },
            ticks: {
              color: colors.text,
              callback: (value: number) => formatPrice(value * 100),
            },
          },
        },
      },
    });
  } catch (error) {
    console.error("Failed to load price chart:", error);
    const container = canvas.parentElement;
    if (container) {
      container.innerHTML = `
        <div class="flex items-center justify-center h-full text-neutral-500">
          Failed to load chart data
        </div>
      `;
    }
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const chartElement = document.getElementById("price-chart");
  if (chartElement) {
    const symbol = chartElement.dataset.symbol;
    if (symbol) {
      loadPriceChart(symbol);
    }
  }
});
