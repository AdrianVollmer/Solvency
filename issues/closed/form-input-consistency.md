# Standardize form input styling

Form inputs across the app use inconsistent styling. Some use the `.input` component class,
others use inline Tailwind classes. This makes maintenance harder and creates visual
inconsistencies.

## Current state

The `.input` class exists in `input.css`:
```css
.input {
    @apply px-4 py-2.5 border border-neutral-300 ...;
    transition: border-color 150ms ease-out, ...;
}
```

But many forms use inline styles:
```html
<input class="w-full px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg...">
```

## Issues

1. Inconsistent colors (`gray-*` vs `neutral-*`)
2. Missing hover states on some inputs
3. Different padding values
4. No transition on some inputs

## Fix

Replace all inline input styles with the `.input` class:

```html
<input type="text" name="name" class="input w-full">
```

## Files to update

- `templates/components/expense_form.html` - uses gray-* colors
- `templates/pages/rules.html` - uses neutral-* but inline
- `templates/pages/tags.html` - uses neutral-* but inline
- `templates/pages/categories.html` - uses neutral-* but inline
- `templates/pages/settings.html`
