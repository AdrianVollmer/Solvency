# Animate the expenses drill-down reveal

When the user shift+drags to select a date range, the top expenses table appears
instantly with no transition. This feels jarring and broken.

## Current state

```typescript
function showTopExpenses(...) {
    // ...
    container.classList.remove("hidden");
    container.scrollIntoView({ behavior: "smooth", block: "nearest" });
}
```

The container goes from `display: none` to visible with no animation.

## Problem

The shift+drag interaction is a deliberate exploratory action. Users expect a
satisfying reveal when they discover hidden functionality. A sudden DOM appearance
feels like something errored rather than a designed experience.

## Suggested fix

Use a slide-down animation with the existing `animate-fade-in` utility or implement
a proper height animation using `grid-template-rows`:

```css
.expenses-reveal {
    display: grid;
    grid-template-rows: 0fr;
    transition: grid-template-rows 200ms var(--ease-out-expo);
}

.expenses-reveal.visible {
    grid-template-rows: 1fr;
}

.expenses-reveal > div {
    overflow: hidden;
}
```

Then update the TypeScript to toggle the `visible` class instead of `hidden`.

## Relevant files

- `static/ts/net-worth-chart.ts` (showTopExpenses, hideTopExpenses functions)
- `templates/pages/net_worth.html` (container markup)
- `static/css/input.css` (animation utilities)
