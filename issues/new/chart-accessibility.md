# Chart Accessibility

The analytics page uses Chart.js canvas elements with `aria-label` attributes, but this is
insufficient for screen reader users who cannot perceive the visual data.

## Current state

```html
<canvas id="category-chart" aria-label="Pie chart showing spending by category"></canvas>
```

## Required improvements

1. Add a visually hidden data table as fallback for each chart
2. Or use `aria-describedby` to link to a summary of the data
3. Consider adding a "View as table" toggle for accessibility

## Relevant files

- `templates/pages/analytics.html`
- `static/js/src/charts.js`

## References

- https://www.w3.org/WAI/tutorials/images/complex/
- https://www.chartjs.org/docs/latest/general/accessibility.html
