# Replace confirm() dialogs with undo pattern

Destructive actions (delete expense, delete category, delete tag, delete rule) currently use
`confirm()` dialogs which interrupt user flow and are not accessible.

## Current pattern

```javascript
if (confirm('Are you sure?')) {
    // delete
}
```

## Better UX: Undo pattern

1. Perform the delete immediately (optimistic UI)
2. Show a toast notification: "Expense deleted. [Undo]"
3. If user clicks Undo within ~5 seconds, restore the item
4. After timeout, permanently delete

This is faster, less disruptive, and follows modern UX best practices.

## Implementation notes

- Add soft-delete support (deleted_at timestamp) or temporary storage
- Create a toast component with action button support
- Wire up HTMX to handle the undo action

## Relevant files

- `templates/components/expense_row.html`
- `templates/components/rule_row.html`
- `templates/components/tag_badge.html`
- `templates/pages/categories.html`
- `static/js/src/main.js` (toast system)
