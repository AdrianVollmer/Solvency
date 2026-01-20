# Remove unnecessary chart wrapper and heading

The price chart is wrapped in a card with a redundant "Price History" heading that
adds no value.

## Current state

```html
<div class="bg-white dark:bg-neutral-800 rounded-xl border border-neutral-200 dark:border-neutral-700 p-6">
    <h2 class="text-lg font-semibold text-neutral-900 dark:text-white mb-4">Price History</h2>
    <div id="position-chart" class="h-80" ...></div>
    <p class="mt-2 text-xs text-neutral-500">
        <span class="inline-block w-3 h-3 rounded-full bg-green-500 mr-1"></span> Buy
        <span class="inline-block w-3 h-3 rounded-full bg-red-500 mr-1 ml-3"></span> Sell
    </p>
</div>
```

## Problems

1. **Redundant heading** — "Price History" states the obvious; users know what a
   stock chart is
2. **Card wrapper reduces prominence** — The chart should be a hero element, not
   boxed in like everything else
3. **Legend is crude** — Colored circles with text labels feel hand-coded, not
   designed
4. **Uniform treatment** — Chart gets the same card styling as the stats and
   activity table

## Proposed fix

### Remove card wrapper

Let the chart stand on its own with whitespace:

```html
<div class="mt-8">
    <div id="position-chart" class="h-80" ...></div>
</div>
```

### Remove or relocate heading

If any label is needed, make it subtle and informative:

```html
<div class="flex items-center justify-between mb-2">
    <p class="text-xs text-neutral-400 uppercase tracking-wider">1Y Price History</p>
    <!-- Optional: time range selector -->
</div>
```

Or remove entirely — the chart is self-explanatory in context.

### Improve legend

Option A: Move legend into chart using ECharts native legend:

```typescript
legend: {
    data: ['Price', 'Buy', 'Sell'],
    bottom: 0,
    textStyle: { fontSize: 11 }
}
```

Option B: Style the HTML legend more elegantly:

```html
<div class="mt-3 flex items-center gap-4 text-xs text-neutral-500">
    <span class="flex items-center gap-1.5">
        <span class="w-2 h-2 rounded-full bg-green-500"></span>
        Buy
    </span>
    <span class="flex items-center gap-1.5">
        <span class="w-2 h-2 rounded-full bg-red-500"></span>
        Sell
    </span>
</div>
```

Option C: Remove legend if cost basis line (from other issue) makes buy points
self-evident.

## Relevant files

- `templates/pages/position_detail.html` (lines 138-146)
- `static/ts/position-chart.ts` (if moving legend into chart)

## Implementation notes

- Removing the card wrapper means the chart needs adequate vertical margin to
  separate it from adjacent sections
- If the chart fails to load, the error state currently shows inside the container;
  ensure it still looks acceptable without the card border
- Consider whether the activity table below should also lose its card wrapper for
  consistency, or keep it since tables benefit from containment
