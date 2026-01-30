Add a Savings Rate Over Time analytics page.

The Spending page already classifies transactions as income (positive amount) or
expenses (negative amount), and the Sankey chart shows income-to-expense flow for
a single period. What's missing is a longitudinal view of how much of the user's
income they are actually retaining each month.

## What to show

A monthly bar or combo chart with three series:

- **Total income** for the month (sum of positive transactions, excluding
  transfers).
- **Total expenses** for the month (sum of negative transactions, excluding
  transfers).
- **Savings rate** as a percentage line: `(income - expenses) / income * 100`.

Include a summary row or card above the chart showing the overall savings rate
across the selected date range, total saved, and the best/worst months.

## Data sources

All data comes from the `transactions` table. Filter out the "Transfers" category
tree (same exclusion logic already used in the spending analytics). Group by
`date` truncated to month. Income = `SUM(amount_cents) WHERE amount_cents > 0`,
expenses = `SUM(amount_cents) WHERE amount_cents < 0`.

## Notes

- Months with zero income should show the savings rate as N/A or skip, not
  divide by zero.
- Consider supporting the same date presets and navigation (prev/next) already
  used on the Spending page.
- This could live as a new tab on the Spending page or as a standalone page
  linked from the sidebar.
