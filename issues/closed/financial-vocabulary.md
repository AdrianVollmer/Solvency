# Financial Vocabulary Issues in HTML Templates

The UI vocabulary should be idiomatic and correct from a financial
perspective. Several templates use terms that are imprecise, ambiguous,
or inconsistent.

## 1. "Expenses" misnomer

**Severity:** significant
**Files:** `sidebar.html`, `expenses.html`, `expense_new.html`,
`dashboard.html`

The app calls the transactions section "Expenses" and uses "Add Expense"
throughout, yet the system also handles income (positive amounts). The
detail page (`expense_detail.html`) correctly uses "Transaction Details"
and distinguishes "Income" vs "Expense", but the list page, sidebar, and
forms all say "Expenses". A user adding their salary has to click "Add
Expense".

**Fix:** Rename to "Transactions" in the sidebar, list page title, and
forms. "Add Expense" becomes "Add Transaction", "Record a new expense"
becomes "Record a new transaction", etc.

## 2. "Gain/Loss" on open positions should be "Unrealized Gain/Loss"

**Severity:** moderate
**File:** `trading_positions.html` (column header)

The column says "Gain/Loss" for open positions. The closed positions
page correctly says "Realized Gain/Loss", and the position detail hero
correctly says "Unrealized Gain/Loss". The main positions table should
say "Unrealized G/L" for consistency and financial correctness.

## 3. "Total expenses" on dashboard is a count, not an amount

**Severity:** moderate
**File:** `dashboard.html`

The label "Total expenses" is paired with `expense_count`, which is a
numeric count. Users will expect a monetary total. Rename to "Expense
count" or "Transactions".

## 4. "shares" is too specific

**Severity:** minor
**File:** `position_detail.html`

The text `{{ quantity }} shares` assumes equities. Not all instruments
use "shares" (ETFs use "units", bonds use "face value", options use
"contracts"). Use "units" instead, or omit the unit label entirely.

## 5. Net worth subtitle is vague

**Severity:** minor
**File:** `net_worth.html`

The subtitle says "Your total net worth over time, combining expenses
and portfolio value". This doesn't clearly convey how the number is
derived. Change to something like "Combining account balances and
portfolio holdings".

## 6. "Price" column on positions is ambiguous

**Severity:** minor
**File:** `trading_positions.html`

The column is labeled just "Price". In financial reporting the standard
label is "Market Price", "Last Price", or "Current Price" (the latter is
already used on the position detail page).

## 7. "Period" should be "Holding Period"

**Severity:** minor
**File:** `trading_positions_closed.html`

The column header "Period" for closed positions shows the date range of
first to last activity. In finance, this is the "holding period".

## 8. "Total Value" on activity detail is ambiguous

**Severity:** minor
**File:** `trading_activity_detail.html`

For a BUY, the total is the cost; for a SELL, it's the proceeds. "Total
Value" could be confused with current market value. Most brokerage
statements use "Total Amount" or simply "Amount".

## 9. "Symbols Needing Data" is awkward

**Severity:** cosmetic
**File:** `market_data.html`

"Symbols Needing Data" reads awkwardly. "Missing Data" or "Symbols
Without Data" would be more natural.

## 10. "Counterparty" is overly technical for personal finance

**Severity:** cosmetic
**File:** `expense_table.html` (partial)

"Counterparty" is correct in institutional finance but unusual in
personal finance apps. Users expect "Payee" or "Merchant". The detail
page already has separate "Payer" and "Payee" fields. The table column
could use "Payee" instead.
