# Add loading state for Net Worth chart

The chart container is empty until data loads, showing a blank white/dark rectangle.
There's no indication that content is loading.

## Current state

```html
<div id="net-worth-chart" style="height: 500px;" ...></div>
```

The container sits empty during the fetch. If the API is slow, users see nothing
and may think the page is broken.

## Problem

Loading states reduce perceived wait time and reassure users that something is
happening. A 500px tall empty box provides no feedback.

## Suggested fixes

1. Add a skeleton loader that mimics the chart shape:
   ```html
   <div id="net-worth-chart" class="relative" style="height: 500px;">
       <div class="chart-skeleton absolute inset-0 flex items-end gap-1 p-4">
           <!-- Animated bars suggesting a chart -->
       </div>
   </div>
   ```

2. Or a simple centered spinner/pulse indicator

3. The TypeScript should remove/hide the skeleton once the chart initializes:
   ```typescript
   const skeleton = container.querySelector('.chart-skeleton');
   if (skeleton) skeleton.remove();
   netWorthChart = echarts.init(container, getTheme());
   ```

4. Also handle the error state more gracefully - the current error message is
   plain text centered in a div. Consider a proper empty state with retry action.

## Relevant files

- `templates/pages/net_worth.html`
- `static/ts/net-worth-chart.ts`
- `static/css/input.css` (skeleton animation)
