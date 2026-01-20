# Remove unnecessary card wrapper from Net Worth chart

The chart is wrapped in a card container identical to every other section on the page,
creating visual monotony and reducing the chart's prominence as the hero element.

## Current state

```html
<div class="bg-white dark:bg-neutral-800 rounded-xl border border-neutral-200 dark:border-neutral-700 overflow-hidden">
    <div class="p-4">
        <div id="net-worth-chart" style="height: 500px;"></div>
    </div>
    <div class="px-6 py-3 border-t ...">
        <p class="text-xs text-muted">Click the legend items...</p>
    </div>
</div>
```

## Problem

When everything looks the same, nothing stands out. The chart should be the star of
this page, but it gets the same visual treatment as a secondary stats section. The
card wrapper adds unnecessary borders and padding that distance the user from the data.

## Suggested fix

1. Remove the card wrapper from the chart
2. Let the chart stand alone with generous whitespace
3. Move the help text to a more subtle location (tooltip, info icon, or remove if
   the interaction is discoverable enough)
4. Keep the card treatment only for the drill-down expenses table (which benefits
   from containment since it appears/disappears dynamically)

## Relevant files

- `templates/pages/net_worth.html`
- `static/css/input.css` (if new utility classes needed)
