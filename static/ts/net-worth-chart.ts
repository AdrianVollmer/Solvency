declare const echarts: any;

interface NetWorthChartResponse {
  labels: string[];
  net_worth: number[];
  expense_component: number[];
  portfolio_component: number[];
}

interface ExpenseItem {
  id: number;
  date: string;
  description: string;
  amount_cents: number;
  currency: string;
  category_name: string | null;
  category_color: string | null;
}

interface TopExpensesResponse {
  expenses: ExpenseItem[];
  from_date: string;
  to_date: string;
}

let netWorthChart: any = null;
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

async function fetchTopExpenses(fromDate: string, toDate: string): Promise<TopExpensesResponse> {
  const params = new URLSearchParams({ from_date: fromDate, to_date: toDate });
  const response = await fetch(`/api/net-worth/top-expenses?${params}`);
  if (!response.ok) throw new Error("Failed to fetch expenses");
  return response.json();
}

function showTopExpenses(data: TopExpensesResponse, currency: string, locale: string): void {
  const container = document.getElementById("top-expenses-container");
  const periodEl = document.getElementById("top-expenses-period");
  const tbody = document.getElementById("top-expenses-body");

  if (!container || !periodEl || !tbody) return;

  periodEl.textContent = `${data.from_date} to ${data.to_date}`;

  tbody.innerHTML = data.expenses
    .map((expense) => {
      const amountClass =
        expense.amount_cents >= 0
          ? "text-green-600 dark:text-green-400"
          : "text-red-600 dark:text-red-400";
      const categoryBadge = expense.category_name
        ? `<span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium" style="background-color: ${expense.category_color || "#6b7280"}20; color: ${expense.category_color || "#6b7280"}">${expense.category_name}</span>`
        : '<span class="text-neutral-400">-</span>';

      return `
        <tr>
          <td class="px-6 py-4 whitespace-nowrap text-sm text-neutral-900 dark:text-white">${expense.date}</td>
          <td class="px-6 py-4 text-sm text-neutral-900 dark:text-white">
            <a href="/expenses/${expense.id}" class="hover:text-blue-600 dark:hover:text-blue-400">${expense.description}</a>
          </td>
          <td class="px-6 py-4 whitespace-nowrap text-sm">${categoryBadge}</td>
          <td class="px-6 py-4 whitespace-nowrap text-sm font-medium text-right ${amountClass}">${formatMoney(expense.amount_cents, currency, locale)}</td>
        </tr>
      `;
    })
    .join("");

  container.classList.remove("hidden");
  container.scrollIntoView({ behavior: "smooth", block: "nearest" });
}

function hideTopExpenses(): void {
  const container = document.getElementById("top-expenses-container");
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
          fillerColor: isDarkMode() ? "rgba(59, 130, 246, 0.2)" : "rgba(59, 130, 246, 0.1)",
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

    // Handle brush selection end
    netWorthChart.on("brushEnd", async (params: any) => {
      if (!params.areas || params.areas.length === 0) return;

      const area = params.areas[0];
      if (!area.coordRange || area.coordRange.length < 2) return;

      const [startIdx, endIdx] = area.coordRange;
      const fromDate = chartLabels[Math.floor(startIdx)];
      const toDate = chartLabels[Math.min(Math.ceil(endIdx), chartLabels.length - 1)];

      if (fromDate && toDate) {
        try {
          const expensesData = await fetchTopExpenses(fromDate, toDate);
          showTopExpenses(expensesData, currency, locale);
        } catch (error) {
          console.error("Failed to fetch top expenses:", error);
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

    // Close button for expenses table
    const closeBtn = document.getElementById("top-expenses-close");
    if (closeBtn) {
      closeBtn.addEventListener("click", hideTopExpenses);
    }
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
