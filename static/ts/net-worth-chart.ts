declare const echarts: any;

interface NetWorthChartResponse {
  labels: string[];
  net_worth: number[];
  expense_component: number[];
  portfolio_component: number[];
}

let netWorthChart: any = null;

function getCurrencySymbol(currency: string): string {
  const symbols: Record<string, string> = {
    USD: "$",
    EUR: "€",
    GBP: "£",
    JPY: "¥",
    CNY: "¥",
    CAD: "C$",
    AUD: "A$",
    CHF: "CHF ",
    INR: "₹",
    BRL: "R$",
    MXN: "MX$",
    KRW: "₩",
    SEK: "kr ",
    NOK: "kr ",
    DKK: "kr ",
    PLN: "zł ",
    RUB: "₽",
    TRY: "₺",
    ZAR: "R ",
    SGD: "S$",
    HKD: "HK$",
    NZD: "NZ$",
    THB: "฿",
  };
  return symbols[currency.toUpperCase()] || "$";
}

function formatMoney(cents: number, currency: string, locale: string): string {
  const value = cents / 100;
  const symbol = getCurrencySymbol(currency);
  return (
    symbol +
    value.toLocaleString(locale, {
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    })
  );
}

function isDarkMode(): boolean {
  return document.documentElement.classList.contains("dark");
}

function getTheme(): string | undefined {
  return isDarkMode() ? "dark" : undefined;
}

async function loadNetWorthChart(): Promise<void> {
  const container = document.getElementById("net-worth-chart");
  if (!container) return;

  const currency = container.dataset.currency || "USD";
  const locale = container.dataset.locale || "en-US";

  try {
    const response = await fetch("/api/net-worth/chart");
    if (!response.ok) throw new Error("Failed to fetch data");

    const data: NetWorthChartResponse = await response.json();

    if (netWorthChart) {
      netWorthChart.dispose();
    }

    netWorthChart = echarts.init(container, getTheme());

    // Convert cents to dollars for display
    const netWorthDollars = data.net_worth.map((c) => c / 100);
    const expenseDollars = data.expense_component.map((c) => c / 100);
    const portfolioDollars = data.portfolio_component.map((c) => c / 100);

    const showSymbols = data.labels.length <= 100;

    const option = {
      tooltip: {
        trigger: "axis",
        formatter: (params: any) => {
          let result = `<strong>${params[0].axisValue}</strong><br/>`;
          for (const param of params) {
            const value = formatMoney(param.value * 100, currency, locale);
            result += `${param.marker} ${param.seriesName}: ${value}<br/>`;
          }
          return result;
        },
      },
      legend: {
        data: ["Net Worth", "Expenses (Cumulative)", "Portfolio Value"],
        top: 0,
        selected: {
          "Net Worth": true,
          "Expenses (Cumulative)": false,
          "Portfolio Value": false,
        },
      },
      grid: {
        left: "3%",
        right: "4%",
        bottom: "3%",
        top: 40,
        containLabel: true,
      },
      xAxis: {
        type: "category",
        boundaryGap: false,
        data: data.labels,
        axisLabel: {
          rotate: 45,
        },
      },
      yAxis: {
        type: "value",
        axisLabel: {
          formatter: (value: number) => formatMoney(value * 100, currency, locale),
        },
      },
      series: [
        {
          name: "Net Worth",
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
            opacity: isDarkMode() ? 0.15 : 0.1,
          },
          symbol: showSymbols ? "circle" : "none",
          symbolSize: 4,
          data: netWorthDollars,
          z: 3,
        },
        {
          name: "Expenses (Cumulative)",
          type: "line",
          smooth: true,
          lineStyle: {
            width: 1.5,
            color: "#22c55e",
            type: "dashed",
          },
          itemStyle: {
            color: "#22c55e",
          },
          symbol: "none",
          data: expenseDollars,
          z: 2,
        },
        {
          name: "Portfolio Value",
          type: "line",
          smooth: true,
          lineStyle: {
            width: 1.5,
            color: "#f59e0b",
            type: "dashed",
          },
          itemStyle: {
            color: "#f59e0b",
          },
          symbol: "none",
          data: portfolioDollars,
          z: 1,
        },
      ],
    };

    netWorthChart.setOption(option);

    window.addEventListener("resize", () => {
      if (netWorthChart) netWorthChart.resize();
    });
  } catch (error) {
    console.error("Failed to load net worth chart:", error);
    container.innerHTML = `
      <div class="flex items-center justify-center h-full text-neutral-500">
        Failed to load chart data.
      </div>
    `;
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const chartElement = document.getElementById("net-worth-chart");
  if (chartElement) {
    loadNetWorthChart();
  }
});
