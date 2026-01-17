# Ensure minimum touch target sizes

Some interactive elements have touch targets smaller than the WCAG recommended 44x44px
minimum, making them difficult to tap on mobile devices.

## Problem areas

1. **Pagination arrows** in expenses/analytics - using `p-2.5` (~40px)
2. **Icon-only buttons** - close buttons, theme toggle
3. **Checkbox/radio inputs** - default browser size is too small
4. **Tag checkboxes** in expense form

## WCAG 2.2 Success Criterion 2.5.8

Target size should be at least 44x44 CSS pixels, or have sufficient spacing.

## Fixes

1. Increase padding on icon buttons to `p-3` (48px) or ensure 44px minimum
2. For checkboxes, increase the clickable label area
3. Add `min-h-[44px] min-w-[44px]` to small interactive elements

## Example fix

```html
<!-- Before -->
<button class="p-2 hover:bg-neutral-100">
    <svg class="w-5 h-5">...</svg>
</button>

<!-- After -->
<button class="p-3 hover:bg-neutral-100 min-w-[44px] min-h-[44px] flex items-center justify-center">
    <svg class="w-5 h-5">...</svg>
</button>
```

## Relevant files

- `templates/pages/expenses.html`
- `templates/pages/analytics.html`
- `templates/components/navbar.html`
- `templates/components/expense_form.html`
