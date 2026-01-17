# Add keyboard shortcuts for power users

The app lacks keyboard shortcuts for common actions, which slows down power users who
prefer keyboard navigation.

## Suggested shortcuts

| Shortcut | Action |
|----------|--------|
| `g d` | Go to Dashboard |
| `g e` | Go to Expenses |
| `g a` | Go to Analytics |
| `g i` | Go to Import |
| `g s` | Go to Settings |
| `n` | New expense (on expenses page) |
| `/` | Focus search input |
| `?` | Show keyboard shortcuts help |
| `Esc` | Close modal/sidebar |

## Implementation

1. Create a keyboard shortcut handler in main.js
2. Use a two-key sequence for navigation (`g` then `d`) to avoid conflicts
3. Show shortcuts in a help modal (triggered by `?`)
4. Respect input focus (don't trigger shortcuts when typing)

```javascript
let pendingKey = null;

document.addEventListener('keydown', (e) => {
    // Don't trigger in inputs
    if (e.target.matches('input, textarea, select')) return;

    if (pendingKey === 'g') {
        if (e.key === 'd') window.location = '/';
        if (e.key === 'e') window.location = '/expenses';
        // ...
        pendingKey = null;
    } else if (e.key === 'g') {
        pendingKey = 'g';
        setTimeout(() => pendingKey = null, 1000);
    }
});
```

## Relevant files

- `static/js/src/main.js`
- `templates/base.html` (help modal)
