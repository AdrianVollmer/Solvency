# Minor polish items for position detail page

A collection of smaller improvements that don't warrant individual issues.

## 1. Add loading state for chart

**Current:** Empty container until data arrives
**Problem:** 320px of blank space with no feedback

**Fix:** Add skeleton or spinner:

```html
<div id="position-chart" class="h-80 relative" ...>
    <div class="chart-skeleton absolute inset-0 flex items-center justify-center">
        <div class="animate-pulse text-neutral-400">Loading chart...</div>
    </div>
</div>
```

```typescript
// In loadPositionChart()
const skeleton = container.querySelector('.chart-skeleton');
if (skeleton) skeleton.remove();
```

**File:** `templates/pages/position_detail.html`, `static/ts/position-chart.ts`

---

## 2. Improve error state for chart

**Current:**
```html
<div class="flex items-center justify-center h-full text-neutral-500">
    Failed to load chart data. <a href="..." class="text-blue-500 hover:underline ml-1">Fetch market data</a>
</div>
```

**Problem:** Generic error message, styling inconsistent with rest of app

**Fix:** Use proper empty state pattern:

```typescript
container.innerHTML = `
    <div class="flex flex-col items-center justify-center h-full text-center">
        <svg class="w-12 h-12 text-neutral-300 mb-3"><!-- chart icon --></svg>
        <p class="text-neutral-600 dark:text-neutral-400 mb-2">No price data available</p>
        <a href="/trading/market-data/${symbol}" class="text-sm text-primary-600 hover:text-primary-700">
            Fetch market data →
        </a>
    </div>
`;
```

**File:** `static/ts/position-chart.ts` (lines 169-176)

---

## 3. Add tabular-nums to all monetary values

**Current:** Some values may not align properly in the stats grid
**Problem:** Variable-width digits cause visual jitter

**Fix:** Add `tabular-nums` class to all monetary displays:

```html
<p class="text-xl font-semibold tabular-nums ...">{{ value }}</p>
```

**File:** `templates/pages/position_detail.html` (all stat cards)

---

## 4. Activity table: Add link to edit transaction

**Current:** Activity rows are display-only
**Problem:** User may want to correct a mistake without leaving the page

**Fix:** Add subtle edit action on hover:

```html
<tr class="group">
    <td>...</td>
    <td>...</td>
    <td class="text-right">
        <a href="/trading/activities/{{ activity.id }}/edit"
           class="opacity-0 group-hover:opacity-100 text-neutral-400 hover:text-neutral-600 transition-opacity">
            Edit
        </a>
    </td>
</tr>
```

Or add a link icon in the date column that goes to the activity detail.

**File:** `templates/pages/position_detail.html` (lines 166-194)

---

## 5. Quote type badge could be more distinctive

**Current:**
```html
<span class="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-neutral-100 dark:bg-neutral-700 text-neutral-600 dark:text-neutral-300">ETF</span>
```

**Problem:** Bland gray badge doesn't differentiate between asset types

**Fix:** Use subtle color coding by type:

```html
{% match symbol_info.quote_type %}
{% when Some with ("ETF") %}
<span class="px-2 py-0.5 rounded text-xs font-medium bg-blue-50 text-blue-700 dark:bg-blue-900/30 dark:text-blue-300">ETF</span>
{% when Some with ("EQUITY") %}
<span class="px-2 py-0.5 rounded text-xs font-medium bg-emerald-50 text-emerald-700 dark:bg-emerald-900/30 dark:text-emerald-300">Stock</span>
{% when Some with (qt) %}
<span class="...neutral...">{{ qt }}</span>
{% when None %}{% endmatch %}
```

**File:** `templates/pages/position_detail.html` (lines 20-23)

---

## 6. Consider adding date range selector to chart

**Current:** Chart shows all available data
**Problem:** Long time periods make recent detail hard to see

**Fix:** Add simple range buttons or integrate dataZoom like net worth page:

```html
<div class="flex gap-2 mb-2">
    <button class="text-xs px-2 py-1 rounded ..." data-range="1m">1M</button>
    <button class="text-xs px-2 py-1 rounded ..." data-range="3m">3M</button>
    <button class="text-xs px-2 py-1 rounded ..." data-range="1y">1Y</button>
    <button class="text-xs px-2 py-1 rounded ..." data-range="all">All</button>
</div>
```

Or add ECharts dataZoom slider like the net worth chart.

**File:** `templates/pages/position_detail.html`, `static/ts/position-chart.ts`

---

## Priority order

1. Loading state for chart (quick win, improves perceived performance)
2. tabular-nums on monetary values (quick win, improves alignment)
3. Error state improvement (moderate effort)
4. Quote type badge colors (quick win)
5. Activity table edit links (depends on whether edit route exists)
6. Date range selector (larger effort, could be separate issue)
