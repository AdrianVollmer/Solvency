# Collapse action bar behind overflow menu on mobile

The `page_action_bar` macro renders 4 buttons (Export, Import, Delete All,
Add) in a flat `flex flex-wrap` row. On mobile (<640px), they wrap into 2
rows of equally-weighted buttons, making the page header area tall and
visually busy. The user's primary action ("Add Account", etc.) competes with
rarely-used bulk operations.

## Problems

1. **No progressive disclosure** (Critical). All 4 actions have near-equal
   visual weight. Export/Import/Delete All are infrequent operations that
   shouldn't compete with the primary "Add" button.
2. **Touch targets too small** (High). With `gap-2` (8px) between buttons and
   `py-2`/`py-2.5` padding, buttons are ~36-40px tall -- below the WCAG 2.2
   SC 2.5.8 recommendation of 44px. Tightly packed destructive and
   non-destructive buttons invite mis-taps.
3. **Awkward sm breakpoint transition** (Medium). The parent container uses
   `flex-col sm:flex-row`. At 640-768px the title and 4-button group go
   side-by-side, but 4 buttons can't fit, so they wrap internally into an
   uneven 2-row block beside the title.

## Suggested approach

- Keep only the primary "Add" button visible at all times.
- Collapse Export, Import, and Delete All behind a "..." (ellipsis) overflow
  menu on viewports below `md` (768px).
- On desktop, the current layout is acceptable (all 4 fit in one row), but
  could still benefit from the overflow menu for visual calm.
- Consider raising the stacking breakpoint from `sm` to `md` or `lg`.

## WCAG references

- SC 2.5.8 Target Size (Minimum) -- 24px
- SC 2.5.5 Target Size (Enhanced) -- 44px

## Relevant files

- `templates/macros/ui.html` (lines 439-467, `page_action_bar` macro)
- All 6 manage pages that call the macro:
  - `templates/pages/accounts.html`
  - `templates/pages/transactions.html`
  - `templates/pages/categories.html`
  - `templates/pages/tags.html`
  - `templates/pages/rules.html`
  - `templates/pages/trading_activities.html`
