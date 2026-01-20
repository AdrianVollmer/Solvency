# Add financial context to position chart

The price chart shows historical prices with buy/sell markers, but lacks the context
needed to answer "how am I doing?" at a glance.

## Current state

```typescript
// static/ts/position-chart.ts
const option = {
    series: [{
        type: "line",
        lineStyle: { color: "#3b82f6" },
        areaStyle: { opacity: 0.1 },
        data: prices,
        markPoint: { data: markPointData }  // buy/sell dots
    }]
};
```

The chart shows:
- Price line in generic blue (#3b82f6)
- Area fill below the line
- Green/red dots for buy/sell transactions

The chart does NOT show:
- Cost basis (the most important reference point)
- Whether user is in profit or loss
- Entry points emphasized beyond small dots

## Problems

1. **No cost basis reference** — User must mentally compare current price to their
   average cost; this should be visual
2. **No profit/loss visualization** — The area fill is uniform; it should show
   profit zone vs. loss zone
3. **Generic styling** — Blue line chart looks like every other stock chart
4. **Buy markers blend in** — Small dots on a long time series are easy to miss

## Proposed enhancements

### 1. Add cost basis line

Draw a horizontal line at the average cost per share:

```typescript
const avgCostPerShare = /* pass from backend */;

series: [
    // Price line
    { ... },
    // Cost basis reference line
    {
        type: "line",
        markLine: {
            silent: true,
            symbol: "none",
            lineStyle: {
                color: isDarkMode() ? "#525252" : "#a3a3a3",
                type: "dashed",
                width: 1
            },
            data: [{
                yAxis: avgCostPerShare,
                label: {
                    formatter: "Cost basis",
                    position: "insideEndTop"
                }
            }]
        }
    }
]
```

### 2. Shade profit/loss zones

Use different area colors above vs. below cost basis:

```typescript
// Use visualMap to color areas differently
visualMap: {
    show: false,
    pieces: [
        { lt: avgCostPerShare, color: "rgba(239, 68, 68, 0.1)" },  // loss zone
        { gte: avgCostPerShare, color: "rgba(34, 197, 94, 0.1)" }  // profit zone
    ]
}
```

Alternative: Just tint the entire area based on current P/L status.

### 3. Emphasize entry points

Make buy transactions more prominent:

```typescript
// For first buy (entry point), use larger marker
const entryPoint = chartData.activities.find(a => a.activity_type === "BUY");

markPointData.push({
    coord: [entryPoint.date, priceAtDate],
    symbol: "pin",
    symbolSize: 20,
    itemStyle: { color: "#22c55e" },
    label: { show: true, formatter: "Entry" }
});
```

### 4. Backend changes needed

Pass additional data to the chart:

```rust
// In the API response for /api/positions/{symbol}/chart
struct ChartResponse {
    symbol: String,
    data: Vec<PriceData>,
    activities: Vec<ActivityMarker>,
    // NEW:
    avg_cost_cents: Option<i64>,
    current_value_cents: Option<i64>,
    gain_loss_cents: Option<i64>,
}
```

## Visual mockup

```
Price
  │
  │                              ╭──────── Current: €74.29
  │                         ╭────╯
  │    ████████████████████╱█████  ← Green area (profit zone)
  │   ╱
  │──────────────────────────────── Cost basis: €45.23
  │  ╱
  │ ████  ← Red area (loss zone, if any)
  │╱
  └────────────────────────────────► Time
       ▲
       Entry point (emphasized)
```

## Relevant files

- `static/ts/position-chart.ts` (main chart logic)
- `src/handlers/position_detail.rs` (API endpoint)
- `templates/pages/position_detail.html` (chart container)

## Implementation notes

- Cost basis should come from `position.average_cost_cents()`
- If cost basis is unavailable (e.g., no position), skip the reference line
- The profit/loss shading is optional if too complex; the cost basis line alone
  adds significant value
- Consider adding a dataZoom slider for longer time periods (like net worth chart)
- Keep the buy/sell legend below the chart but consider making it part of the
  chart's native legend
