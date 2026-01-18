declare const Chart: any;

interface PriceData {
  date: string;
  price_cents: number;
}

interface ActivityMarker {
  date: string;
  activity_type: string;
  quantity: number;
  price_cents: number;
  total_cents: number;
}

interface ChartResponse {
  symbol: string;
  data: PriceData[];
  activities: ActivityMarker[];
}

let positionChart: any = null;

function formatPrice(cents: number): string {
  return "$" + (cents / 100).toFixed(2);
}

function formatQuantity(qty: number): string {
  return qty.toFixed(4).replace(/\.?0+$/, "");
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

async function loadPositionChart(symbol: string): Promise<void> {
  const canvas = document.getElementById("position-chart") as HTMLCanvasElement;
  if (!canvas) return;

  try {
    const response = await fetch(`/api/positions/${encodeURIComponent(symbol)}/chart`);
    if (!response.ok) throw new Error("Failed to fetch data");

    const chartData: ChartResponse = await response.json();
    const colors = getChartColors();

    if (positionChart) {
      positionChart.destroy();
    }

    // Create a map of date -> price for quick lookup
    const priceMap = new Map<string, number>();
    for (const d of chartData.data) {
      priceMap.set(d.date, d.price_cents / 100);
    }

    // Create annotations for buy/sell activities
    const annotations: any = {};
    for (const [i, activity] of chartData.activities.entries()) {
      // Find the price at this date, or use the activity price
      const priceAtDate = priceMap.get(activity.date) ?? activity.price_cents / 100;

      const isBuy = activity.activity_type === "BUY";
      const color = isBuy ? "#22c55e" : "#ef4444"; // green for buy, red for sell

      annotations[`activity${i}`] = {
        type: "point",
        xValue: activity.date,
        yValue: priceAtDate,
        backgroundColor: color,
        borderColor: color,
        borderWidth: 2,
        radius: 6,
        label: {
          display: false,
        },
      };
    }

    positionChart = new Chart(canvas, {
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
              label: (context: any) => {
                const date = context.label;
                const price = formatPrice(context.raw * 100);

                // Check if there's an activity on this date
                const activity = chartData.activities.find((a) => a.date === date);
                if (activity) {
                  const type = activity.activity_type === "BUY" ? "Buy" : "Sell";
                  const qty = formatQuantity(activity.quantity);
                  const total = formatPrice(activity.total_cents);
                  return [
                    `Price: ${price}`,
                    `${type}: ${qty} shares @ ${formatPrice(activity.price_cents)}`,
                    `Total: ${total}`,
                  ];
                }

                return `Price: ${price}`;
              },
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
    console.error("Failed to load position chart:", error);
    const container = canvas.parentElement;
    if (container) {
      container.innerHTML = `
        <div class="flex items-center justify-center h-full text-neutral-500">
          Failed to load chart data. <a href="/trading/market-data/${symbol}" class="text-blue-500 hover:underline ml-1">Fetch market data</a>
        </div>
      `;
    }
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const chartElement = document.getElementById("position-chart");
  if (chartElement) {
    const symbol = chartElement.dataset.symbol;
    if (symbol) {
      loadPositionChart(symbol);
    }
  }
});
