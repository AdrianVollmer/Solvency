declare const echarts: any;

interface NetWorthChartResponse {
  labels: string[];
  net_worth: number[];
  transaction_component: number[];
  portfolio_component: number[];
}

interface AllocationNode {
  name: string;
  color: string;
  amount_cents?: number;
  children: AllocationNode[];
}

let netWorthChart: any = null;
let allocationChart: any = null;
let chartLabels: string[] = [];
let isShiftPressed = false;

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

function updateBrushMode(): void {
  if (!netWorthChart) return;

  if (isShiftPressed) {
    netWorthChart.dispatchAction({
      type: "takeGlobalCursor",
      key: "brush",
      brushOption: {
        brushType: "lineX",
        brushMode: "single",
      },
    });
  } else {
    netWorthChart.dispatchAction({
      type: "takeGlobalCursor",
      key: "brush",
      brushOption: {
        brushType: false,
      },
    });
  }
}

async function fetchTopTransactionsHtml(
  fromDate: string,
  toDate: string,
): Promise<string> {
  const params = new URLSearchParams({ from_date: fromDate, to_date: toDate });
  const response = await fetch(`/api/net-worth/top-transactions?${params}`);
  if (!response.ok) throw new Error("Failed to fetch transactions");
  return response.text();
}

function showTopTransactions(html: string): void {
  const container = document.getElementById("top-transactions-container");
  if (!container) return;

  container.innerHTML = html;
  container.classList.remove("hidden");
  container.scrollIntoView({ behavior: "smooth", block: "nearest" });

  const closeBtn = container.querySelector(".preview-close-btn");
  if (closeBtn) {
    closeBtn.addEventListener("click", hideTopTransactions);
  }
}

function hideTopTransactions(): void {
  const container = document.getElementById("top-transactions-container");
  if (container) {
    container.classList.add("hidden");
  }
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
    chartLabels = data.labels;

    if (netWorthChart) {
      netWorthChart.dispose();
    }

    netWorthChart = echarts.init(container, getTheme());

    // Convert cents to dollars for display
    const netWorthDollars = data.net_worth.map((c) => c / 100);
    const transactionDollars = data.transaction_component.map((c) => c / 100);
    const portfolioDollars = data.portfolio_component.map((c) => c / 100);

    const showSymbols = data.labels.length <= 100;

    const option = {
      backgroundColor: "transparent",
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
        data: ["Net Worth", "Transactions (Cumulative)", "Portfolio Value"],
        top: 0,
        selected: {
          "Net Worth": true,
          "Transactions (Cumulative)": false,
          "Portfolio Value": false,
        },
      },
      toolbox: {
        show: false,
      },
      brush: {
        toolbox: ["lineX"],
        xAxisIndex: 0,
        brushStyle: {
          borderWidth: 1,
          color: "rgba(59, 130, 246, 0.15)",
          borderColor: "rgba(59, 130, 246, 0.5)",
        },
        outOfBrush: {
          colorAlpha: 0.3,
        },
      },
      grid: {
        left: "3%",
        right: "4%",
        bottom: 60,
        top: 40,
        containLabel: true,
      },
      dataZoom: [
        {
          type: "slider",
          xAxisIndex: 0,
          start: 0,
          end: 100,
          height: 30,
          bottom: 10,
          borderColor: isDarkMode() ? "#374151" : "#e5e7eb",
          backgroundColor: isDarkMode() ? "#1f2937" : "#f9fafb",
          fillerColor: isDarkMode()
            ? "rgba(59, 130, 246, 0.2)"
            : "rgba(59, 130, 246, 0.1)",
          handleStyle: {
            color: "#3b82f6",
          },
          textStyle: {
            color: isDarkMode() ? "#e5e7eb" : "#374151",
          },
        },
        {
          type: "inside",
          xAxisIndex: 0,
          start: 0,
          end: 100,
        },
      ],
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
          formatter: (value: number) =>
            formatMoney(value * 100, currency, locale),
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
          name: "Transactions (Cumulative)",
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
          data: transactionDollars,
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

    // Handle brush selection end
    netWorthChart.on("brushEnd", async (params: any) => {
      if (!params.areas || params.areas.length === 0) return;

      const area = params.areas[0];
      if (!area.coordRange || area.coordRange.length < 2) return;

      const [startIdx, endIdx] = area.coordRange;
      const fromDate = chartLabels[Math.floor(startIdx)];
      const toDate =
        chartLabels[Math.min(Math.ceil(endIdx), chartLabels.length - 1)];

      if (fromDate && toDate) {
        try {
          const html = await fetchTopTransactionsHtml(fromDate, toDate);
          showTopTransactions(html);
        } catch (error) {
          console.error("Failed to fetch top transactions:", error);
        }
      }

      // Clear the brush after selection
      netWorthChart.dispatchAction({
        type: "brush",
        areas: [],
      });
    });

    window.addEventListener("resize", () => {
      if (netWorthChart) netWorthChart.resize();
    });

    // Track shift key state
    document.addEventListener("keydown", (e) => {
      if (e.key === "Shift" && !isShiftPressed) {
        isShiftPressed = true;
        updateBrushMode();
      }
    });

    document.addEventListener("keyup", (e) => {
      if (e.key === "Shift") {
        isShiftPressed = false;
        updateBrushMode();
      }
    });

    // Close button wired dynamically in showTopTransactions()
  } catch (error) {
    console.error("Failed to load net worth chart:", error);
    container.innerHTML = `
      <div class="flex items-center justify-center h-full text-neutral-500">
        Failed to load chart data.
      </div>
    `;
  }
}

function mapAllocationToSunburst(nodes: AllocationNode[]): any[] {
  return nodes.map((node) => {
    if (node.children.length > 0) {
      return {
        name: node.name,
        itemStyle: { color: node.color },
        children: mapAllocationToSunburst(node.children),
      };
    }
    return {
      name: node.name,
      value: (node.amount_cents || 0) / 100,
      itemStyle: { color: node.color },
    };
  });
}

async function loadAllocationChart(): Promise<void> {
  const container = document.getElementById("allocation-chart");
  if (!container) return;

  const currency = container.dataset.currency || "USD";
  const locale = container.dataset.locale || "en-US";

  try {
    const response = await fetch("/api/net-worth/account-allocation");
    if (!response.ok) throw new Error("Failed to fetch data");

    const data: AllocationNode[] = await response.json();

    if (data.length === 0) {
      container.innerHTML =
        '<div class="flex items-center justify-center h-full min-h-[200px] text-neutral-400 dark:text-neutral-500 text-sm">' +
        "No account data available" +
        "</div>";
      return;
    }

    if (allocationChart) {
      allocationChart.dispose();
    }

    const dark = isDarkMode();
    allocationChart = echarts.init(container, getTheme());
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
          return `${params.name}: ${formatMoney(value * 100, currency, locale)}`;
        },
      },
      series: [
        {
          type: "sunburst",
          radius: ["0%", "90%"],
          data: mapAllocationToSunburst(data),
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
              itemStyle: { opacity: 1 },
              label: { rotate: "radial", fontSize: 12 },
            },
            {
              r0: "50%",
              r: "90%",
              itemStyle: { opacity: 0.75 },
              label: { align: "right", fontSize: 10 },
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

    allocationChart.setOption(option);

    window.addEventListener("resize", () => {
      if (allocationChart) allocationChart.resize();
    });
  } catch (error) {
    console.error("Failed to load allocation chart:", error);
    container.innerHTML =
      '<div class="flex items-center justify-center h-full text-neutral-500">' +
      "Failed to load allocation data." +
      "</div>";
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const chartElement = document.getElementById("net-worth-chart");
  if (chartElement) {
    loadNetWorthChart();
  }

  const allocationElement = document.getElementById("allocation-chart");
  if (allocationElement) {
    loadAllocationChart();
  }
});
