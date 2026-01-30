Add dividend income and yield tracking.

Trading activities already record Dividend events with amounts, symbols, and
dates. Currently these only appear as line items on individual position detail
pages. There is no aggregated view of dividend income across the portfolio.

## What to show

1. **Dividend income over time**: A monthly or quarterly bar chart showing total
   dividend income received. Optionally stack by symbol to show which positions
   contribute the most.

2. **Per-position dividend summary table**: For each symbol that has ever paid a
   dividend, show:
   - Total dividends received.
   - Yield on cost (total dividends / total cost basis).
   - Most recent dividend date and amount.
   - Number of dividend payments.

3. **Portfolio-level summary cards**:
   - Total dividend income (all time and for selected period).
   - Portfolio yield on cost (total dividends / total portfolio cost).
   - Average monthly/quarterly dividend income.

## Data sources

All data comes from `trading_activities` where `activity_type = 'DIVIDEND'`. The
`unit_price_cents` field holds the dividend amount, `symbol` identifies the
position, and `date` gives the payment date. Cost basis comes from the existing
position calculation logic in `src/db/queries/trading.rs` (`get_positions`).

For yield calculations on closed positions, use `get_closed_positions()` which
already tracks total cost and includes dividends in its gain/loss computation.

## Notes

- This is most naturally a new tab or section on the Positions page, or a
  standalone page linked from the trading section of the sidebar.
- Positions with no dividend history should be excluded from the table.
- Consider both open and closed positions — a user may want to see total
  historical dividend income including from positions they've since sold.
