declare const echarts: any;

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
  is_approximated: boolean;
}

let positionChart: any = null;

function formatPrice(cents: number): string {
  return "$" + (cents / 100).toFixed(2);
}

function formatQuantity(qty: number): string {
  return qty.toFixed(4).replace(/\.?0+$/, "");
}

function isDarkMode(): boolean {
  return document.documentElement.classList.contains("dark");
}

function getTheme(): string | undefined {
  return isDarkMode() ? "dark" : undefined;
}

async function loadPositionChart(symbol: string): Promise<void> {
  const container = document.getElementById("position-chart");
  if (!container) return;

  try {
    const response = await fetch(`/api/positions/${encodeURIComponent(symbol)}/chart`);
    if (!response.ok) throw new Error("Failed to fetch data");

    const chartData: ChartResponse = await response.json();

    if (positionChart) {
      positionChart.dispose();
    }

    positionChart = echarts.init(container, getTheme());

    // Create a map of date -> price for quick lookup
    const priceMap = new Map<string, number>();
    for (const d of chartData.data) {
      priceMap.set(d.date, d.price_cents / 100);
    }

    // Create markPoint data for buy/sell activities
    const markPointData = chartData.activities.map((activity) => {
      const priceAtDate = priceMap.get(activity.date) ?? activity.price_cents / 100;
      const isBuy = activity.activity_type === "BUY";

      return {
        coord: [activity.date, priceAtDate],
        symbol: "circle",
        symbolSize: 12,
        itemStyle: {
          color: isBuy ? "#22c55e" : "#ef4444",
          borderColor: isBuy ? "#22c55e" : "#ef4444",
          borderWidth: 2,
        },
        // Store activity data for tooltip
        activity: activity,
      };
    });

    const dates = chartData.data.map((d) => d.date);
    const prices = chartData.data.map((d) => d.price_cents / 100);
    const showSymbols = chartData.data.length <= 100;

    // Create activity lookup map for tooltip
    const activityMap = new Map<string, ActivityMarker>();
    for (const activity of chartData.activities) {
      activityMap.set(activity.date, activity);
    }

    // Use amber/yellow color for approximated data, blue for actual market data
    const lineColor = chartData.is_approximated ? "#d97706" : "#3b82f6";

    const option = {
      tooltip: {
        trigger: "axis",
        formatter: (params: any) => {
          const point = params[0];
          const date = point.axisValue;
          const price = formatPrice(point.value * 100);

          const activity = activityMap.get(date);
          if (activity) {
            const type = activity.activity_type === "BUY" ? "Buy" : "Sell";
            const qty = formatQuantity(activity.quantity);
            const total = formatPrice(activity.total_cents);
            return [
              `<strong>${date}</strong>`,
              `Price: ${price}`,
              `${type}: ${qty} shares @ ${formatPrice(activity.price_cents)}`,
              `Total: ${total}`,
            ].join("<br/>");
          }

          return `<strong>${date}</strong><br/>Price: ${price}`;
        },
      },
      grid: {
        left: "3%",
        right: "4%",
        bottom: "3%",
        top: 20,
        containLabel: true,
      },
      xAxis: {
        type: "category",
        boundaryGap: false,
        data: dates,
        axisLabel: {
          rotate: 45,
        },
      },
      yAxis: {
        type: "value",
        axisLabel: {
          formatter: (value: number) => formatPrice(value * 100),
        },
      },
      series: [
        {
          name: `${symbol} Price`,
          type: "line",
          smooth: !chartData.is_approximated,
          step: chartData.is_approximated ? "end" : false,
          lineStyle: {
            width: 2,
            color: lineColor,
          },
          itemStyle: {
            color: lineColor,
          },
          areaStyle: {
            opacity: 0.1,
          },
          symbol: showSymbols ? "circle" : "none",
          symbolSize: 4,
          data: prices,
          markPoint: {
            data: markPointData,
            label: {
              show: false,
            },
          },
        },
      ],
    };

    positionChart.setOption(option);

    window.addEventListener("resize", () => {
      if (positionChart) positionChart.resize();
    });
  } catch (error) {
    console.error("Failed to load position chart:", error);
    container.innerHTML = `
      <div class="flex items-center justify-center h-full text-neutral-500">
        Failed to load chart data. <a href="/trading/market-data/${symbol}" class="text-blue-500 hover:underline ml-1">Fetch market data</a>
      </div>
    `;
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
