Add recurring expense and subscription detection.

Users often lose track of subscriptions and recurring charges. The transaction
data already contains descriptions, amounts, and dates — enough to detect
repeating patterns without any manual input.

## Detection logic

Group transactions by a normalized description key (lowercased, trimmed,
possibly with trailing numbers/dates stripped). Within each group, look for
transactions that:

1. Have similar absolute amounts (within some tolerance, e.g. 5%, to account for
   price changes or tax variations).
2. Recur at a roughly regular interval — monthly (28-35 days), weekly (5-9
   days), quarterly (85-100 days), or yearly (350-380 days).

A minimum of 3 occurrences should be required before flagging something as
recurring.

## What to show

A table or card list of detected recurring expenses, each showing:

- Description / counterparty name.
- Detected frequency (weekly / monthly / quarterly / yearly).
- Typical amount.
- Last occurrence date.
- Estimated annual cost (amount * frequency).
- Total spent to date on this subscription.

Sort by estimated annual cost descending so the most expensive subscriptions
are at the top.

## Data sources

Query from `transactions` where `amount_cents < 0` (expenses only), excluding
transfers. The `description`, `payee`, and `counterparty_iban` fields can all
help with grouping. The `counterparty_iban` is especially reliable for matching
when available (European bank imports via SEPA).

## Notes

- This is inherently heuristic. False positives are acceptable — it's better to
  surface a possible subscription and let the user dismiss it than to miss one.
- Consider allowing the user to confirm or dismiss detected subscriptions in a
  future iteration, but the first version can be read-only.
- Could be its own page under the analytics section or a card on a dashboard.
