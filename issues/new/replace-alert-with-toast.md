# Replace alert() calls with toast notifications

Several import functions use `alert()` for success/error feedback, which blocks the UI and
provides poor UX.

## Current pattern

```javascript
alert(result.message);
if (result.imported > 0) {
    location.reload();
}
```

Found in:
- `templates/pages/categories.html` - importCategories()
- `templates/pages/tags.html` - importTags()
- `templates/pages/rules.html` - importRules()

## Better pattern

Use the existing toast container (`#toast-container`) with non-blocking notifications:

```javascript
showToast(result.message, result.imported > 0 ? 'success' : 'error');
if (result.imported > 0) {
    // Optionally reload or use HTMX to refresh the list
}
```

## Implementation

1. Create a `showToast(message, type)` utility function in main.js
2. Toast should auto-dismiss after 4-5 seconds
3. Include close button for manual dismissal
4. Support success, error, and info variants

## Relevant files

- `templates/pages/categories.html`
- `templates/pages/tags.html`
- `templates/pages/rules.html`
- `static/js/src/main.js`
- `templates/base.html` (toast container exists)
