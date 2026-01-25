# Fix position header hierarchy

The position detail header has inverted hierarchy: the ticker symbol (shorthand) is
prominent while the human-readable name (actual meaning) is subordinate.

## Current state

```html
<div class="flex-1">
    <div class="flex items-center gap-3">
        <h1 class="page-title">{{ symbol }}</h1>  <!-- VWCE.DE - large -->
        <span class="... text-xs ...">{{ qt }}</span>  <!-- ETF badge -->
        <a href="...yahoo..." class="...">...</a>  <!-- external link -->
    </div>
    <p class="mt-1 text-lg text-neutral-700">{{ name }}</p>  <!-- Company name - smaller -->
    <p class="text-sm text-muted">{{ ex }}</p>  <!-- Exchange - smallest -->
</div>
```

The ticker "VWCE.DE" is `page-title` (2xl font-bold), while the actual name
"Vanguard FTSE All-World UCITS ETF" is just `text-lg`.

## Problem

Tickers are shorthand for people who already know the position. The human-readable
name tells you what you actually own. Current hierarchy serves experts over clarity.

Additionally, the back arrow has no label — just a bare `←` icon that requires
the user to guess where it goes.

## Proposed fix

### Option A: Name as hero (recommended for general users)

```html
<div class="flex items-center gap-4">
    <a href="/trading/positions" class="... text-sm text-neutral-500 hover:text-neutral-700">
        ← Positions
    </a>
</div>

<div class="mt-4">
    <div class="flex items-baseline gap-3">
        <h1 class="text-2xl font-bold text-neutral-900 dark:text-white">
            Vanguard FTSE All-World UCITS ETF
        </h1>
        <a href="https://finance.yahoo.com/quote/VWCE.DE/" ...>
            <svg class="w-4 h-4">...</svg>
        </a>
    </div>
    <div class="mt-1 flex items-center gap-2 text-sm text-neutral-500">
        <span class="font-mono">VWCE.DE</span>
        <span>·</span>
        <span>XETRA</span>
        <span class="px-1.5 py-0.5 bg-neutral-100 rounded text-xs">ETF</span>
    </div>
</div>
```

### Option B: Symbol as hero (for power users)

Make the ticker massive and confident since this is the detail page for that symbol:

```html
<h1 class="text-4xl font-bold tracking-tight font-mono">VWCE.DE</h1>
<p class="text-lg text-neutral-600">Vanguard FTSE All-World UCITS ETF</p>
```

### Back button fix

Always label the back destination:

```html
<a href="/trading/positions" class="inline-flex items-center gap-1.5 text-sm text-neutral-500 hover:text-neutral-900 transition-colors">
    <svg class="w-4 h-4"><!-- arrow --></svg>
    Positions
</a>
```

## Relevant files

- `templates/pages/position_detail.html` (lines 11-39)

## Implementation notes

- If using Option A, handle the case where `symbol_info.display_name()` is `None` —
  fall back to symbol as the main heading
- The quote type badge (ETF, Stock, etc.) should remain but be visually subordinate
- The Yahoo Finance link should stay subtle — it's an escape hatch, not a primary action
- Consider using `font-mono` for the ticker to reinforce that it's a code/identifier
