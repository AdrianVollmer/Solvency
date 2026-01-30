Add counterparty / merchant spending analysis.

Categories answer "what kind of thing did I spend on" but not "who did I spend
it with." A single merchant like a supermarket may span multiple categories, and
a single category like "Shopping" may contain dozens of different merchants.
Grouping by counterparty provides an independent, complementary axis of analysis.

## What to show

- **Top merchants table**: Ranked list of counterparties by total spending in the
  selected period. Each row shows the counterparty name, total amount, number of
  transactions, and average transaction size.
- **Merchant detail**: Clicking a merchant could filter the transaction list to
  show all transactions for that counterparty (or this can be deferred).
- Support the same date range presets and navigation used on the Spending page.

## Grouping logic

Transactions should be grouped by counterparty identity. The best available
identifier depends on the data:

1. `counterparty_iban` — most reliable, unique per entity. Use this as the
   primary grouping key when available.
2. `payee` — next best, provided by many bank exports.
3. `description` — fallback. Needs normalization: lowercase, strip trailing
   dates/reference numbers, collapse whitespace.

When `counterparty_iban` is available, use it for grouping but display the
`payee` or `description` as the human-readable label.

## Data sources

Query from `transactions` where `amount_cents < 0` (expenses only), excluding
transfers. Group by the best available counterparty key. The extended SEPA fields
(`payee`, `counterparty_iban`) from migration 003 are the main enablers here.

## Notes

- Description normalization is inherently imperfect. A first version with simple
  lowercasing and trimming is fine — it doesn't need to be perfect to be useful.
- This could be a new tab on the Spending page or a standalone page.
- Only expense transactions should be included (not income or transfers).
