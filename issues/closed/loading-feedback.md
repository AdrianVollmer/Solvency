# Add loading feedback for async operations

Some operations lack visual feedback while processing, leaving users uncertain if their
action was registered.

## Operations needing loading states

1. **Form submissions** - Add Expense, Edit Expense, Create Rule, etc.
2. **Import operations** - File upload and processing
3. **Delete operations** - If not using optimistic UI
4. **Filter changes** - When HTMX fetches new data

## Current HTMX setup

The CSS already has basic HTMX loading styles:
```css
.htmx-request:not(.htmx-indicator) {
    opacity: 0.7;
}
```

This provides subtle feedback but could be enhanced.

## Improvements

1. **Button loading state**: Disable button and show spinner during submission
   ```html
   <button class="btn btn-primary">
       <span class="htmx-indicator">
           <svg class="animate-spin w-4 h-4">...</svg>
       </span>
       <span>Save</span>
   </button>
   ```

2. **Table skeleton**: Show skeleton rows while loading expenses

3. **Progress indicator**: For file uploads, show upload progress

## Implementation notes

- Use `hx-indicator` to target specific loading elements
- Add a small spinner SVG component
- Consider using `hx-disabled-elt` to disable buttons during request

## Relevant files

- `static/css/input.css` (existing htmx styles)
- `templates/components/expense_form.html`
- `templates/pages/import.html`
- `templates/pages/expenses.html`
