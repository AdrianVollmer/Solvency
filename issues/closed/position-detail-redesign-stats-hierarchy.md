# Redesign position stats with proper visual hierarchy

The position detail page displays 12 metrics in identical cards across 3 rows. This
"data dump" approach treats all information as equally important, creating visual
noise instead of clarity.

## Current state

```html
<!-- Row 1: 4 cards -->
<div class="grid grid-cols-2 sm:grid-cols-4 gap-3">
    <div class="bg-white dark:bg-neutral-800 rounded-lg border ...">
        <p class="text-xs text-neutral-500">Quantity</p>
        <p class="text-xl font-semibold">42.5</p>
    </div>
    <!-- 3 more identical cards: Total Cost, Current Value, Gain/Loss -->
</div>

<!-- Row 2: 4 more identical cards -->
<!-- Row 3: 3 more identical cards -->
```

All 12 cards use the same structure: `text-xs` label, `text-xl font-semibold` value,
same padding (px-4 py-3), same border, same radius.

## Problems

1. **No hierarchy** — Gain/Loss (what users care about most) has the same visual
   weight as Total Fees (supporting detail)
2. **Identical card grid** — Classic AI slop pattern; looks like a template
3. **Organized by database structure** — Rows don't map to user mental model
4. **12 cards create scanning fatigue** — Eye bounces around with no anchor

## Proposed redesign

### 1. Hero the gain/loss
Make unrealized gain/loss the dominant element:
```html
<div class="py-6">
    <p class="text-xs uppercase tracking-wider text-neutral-400">Unrealized Gain/Loss</p>
    <div class="text-4xl font-bold text-green-600 tabular-nums">+€1,234.56</div>
    <p class="text-lg text-green-600">+15.4%</p>
</div>
```

### 2. Group by user question
Reorganize into logical clusters:

**Position (what do I own?)**
- Quantity × Avg Cost = Total Cost

**Current state (what's it worth?)**
- Current Price → Current Value

**Performance (how am I doing?)**
- XIRR (annualized return)
- Realized Gain/Loss

**Costs & income (supporting detail)**
- Fees, Taxes, Dividends — inline text, not cards

### 3. Use varied presentation
- Hero: Large gain/loss with percentage
- Primary stats: 2-3 cards for key figures (quantity, value, XIRR)
- Secondary stats: Inline text row for fees/taxes/dividends
- Remove cards entirely for some metrics

### 4. Example layout

```
┌─────────────────────────────────────────────────────┐
│ UNREALIZED GAIN/LOSS                                │
│ +€1,234.56  (+15.4%)                    [large]     │
└─────────────────────────────────────────────────────┘

42.5 shares @ €45.23 avg = €1,922.78 cost basis

┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│ Current Value│  │ Current Price│  │ XIRR         │
│ €3,157.34    │  │ €74.29       │  │ +18.2%       │
└──────────────┘  └──────────────┘  └──────────────┘

Realized: +€0.00 · Fees: €12.50 · Taxes: €0.00 · Dividends: €45.00
```

## Relevant files

- `templates/pages/position_detail.html` (lines 41-136)
- `static/css/input.css` (may need new utility classes)
- `src/handlers/position_detail.rs` (may need to adjust data passed to template)

## Implementation notes

- The gain/loss hero should use color to reinforce meaning (green/red)
- Use `tabular-nums` for all monetary values for alignment
- Cost basis formula (qty × avg = total) can be inline text, not separate cards
- The "supporting detail" row can use a lighter text color to de-emphasize
