# Remove or replace redundant subtitle

The page subtitle "Your total net worth over time, combining expenses and portfolio
value" tells users what they already know - they navigated here intentionally.

## Current state

```html
<h1 class="page-title">Net Worth</h1>
<p class="mt-1 text-muted">Your total net worth over time, combining expenses and portfolio value</p>
```

## Problem

Every word should earn its place. This subtitle:
- Adds no new information
- Pushes actual content down the page
- Reads like placeholder copy

## Suggested fixes

Option A: Remove entirely. The hero net worth display is now self-explanatory.

Option B: Replace with dynamic, useful context:
- "Up 12% from last month"
- "All-time high reached Dec 15"
- "Tracking since Jan 2024"

Option C: Remove the subtitle and move the period context ("since {{ start_date }}")
that's already shown below the hero number.

## Relevant files

- `templates/pages/net_worth.html`
- `src/handlers/net_worth.rs` (if adding dynamic context like month-over-month change)
