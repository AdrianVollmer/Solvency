declare const echarts: any;

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

function isDarkMode(): boolean {
  return document.documentElement.classList.contains("dark");
}

function getTheme(): string | undefined {
  return isDarkMode() ? "dark" : undefined;
}

async function loadPriceChart(symbol: string): Promise<void> {
  const container = document.getElementById("price-chart");
  if (!container) return;

  try {
    const response = await fetch(`/api/market-data/${encodeURIComponent(symbol)}`);
    if (!response.ok) throw new Error("Failed to fetch data");

    const chartData: ChartResponse = await response.json();

    if (priceChart) {
      priceChart.dispose();
    }

    priceChart = echarts.init(container, getTheme());

    const dates = chartData.data.map((d) => d.date);
    const prices = chartData.data.map((d) => d.price_cents / 100);
    const showSymbols = chartData.data.length <= 100;

    // Create markArea data for missing ranges
    const markAreaData = chartData.missing_ranges.map(([start, end]) => [
      {
        xAxis: start,
        itemStyle: {
          color: "rgba(239, 68, 68, 0.15)",
        },
      },
      {
        xAxis: end,
      },
    ]);

    const option = {
      tooltip: {
        trigger: "axis",
        formatter: (params: any) => {
          const point = params[0];
          return `<strong>${point.axisValue}</strong><br/>${formatPrice(point.value * 100)}`;
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
          smooth: true,
          lineStyle: {
            width: 2,
            color: "#3b82f6",
          },
          itemStyle: {
            color: "#3b82f6",
          },
          areaStyle: {
            opacity: 0.1,
          },
          symbol: showSymbols ? "circle" : "none",
          symbolSize: 4,
          data: prices,
          markArea: {
            silent: true,
            label: {
              show: true,
              position: "inside",
              color: "#ef4444",
              fontSize: 10,
              formatter: "Missing",
            },
            data: markAreaData,
          },
        },
      ],
    };

    priceChart.setOption(option);

    window.addEventListener("resize", () => {
      if (priceChart) priceChart.resize();
    });
  } catch (error) {
    console.error("Failed to load price chart:", error);
    container.innerHTML = `
      <div class="flex items-center justify-center h-full text-neutral-500">
        Failed to load chart data
      </div>
    `;
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
