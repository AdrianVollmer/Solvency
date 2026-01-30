Add spending anomaly detection against historical averages.

The Spending page shows category breakdowns for a chosen period, but it doesn't
tell the user whether a given category's spending is normal or unusual. Comparing
current spending against a historical baseline per category turns the existing
breakdown from descriptive into diagnostic.

## What to show

For each category (leaf or subtree), display:

- Current period spending.
- Historical average for the same period length (e.g. if viewing "This Month",
  compare against the average of the prior 6 months).
- Absolute and percentage deviation from the average.
- A visual indicator: normal (within ~20%), elevated (20-50% above), or high
  (>50% above average). Use color or an icon to convey this.

This could be rendered as:

- An additional column or badge on the existing "By Category" sunburst/table.
- A dedicated "Anomalies" or "Insights" tab on the Spending page that lists only
  the categories with significant deviations, sorted by deviation magnitude.

## Data sources

Reuse `spending_by_category()` from `src/services/analytics.rs`, called twice:
once for the current date range and once for the comparison window. The
comparison window should cover several preceding periods of the same length
(e.g. 6 prior months if viewing a single month). Exclude the current period from
the average so it doesn't dilute the baseline.

## Notes

- Categories with very low historical spending (e.g. < 5 transactions in the
  comparison window) should probably be excluded or shown with a "not enough
  data" qualifier to avoid noisy false positives.
- The comparison window length could be configurable, but a sensible default
  (6 months) is fine for a first version.
- Transfers should be excluded, same as in the existing spending analytics.
