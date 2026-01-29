# Normalize Delete All button to use component classes

The Delete All button in `page_action_bar` uses raw inline Tailwind utility
classes instead of the `.btn` component system that every other button in the
action bar uses. This causes three visible inconsistencies.

## Problems

1. **No focus ring** (High). The Delete All button lacks
   `focus:ring-2 focus:ring-offset-2` because it doesn't inherit from `.btn`.
   Keyboard users cannot see focus on this destructive button. This is a
   WCAG AA violation.
2. **Shorter than sibling buttons** (High). It uses `py-2` (8px) while `.btn`
   uses `py-2.5` (10px), and `icon-xs` (16px) while siblings use `icon-sm`
   (20px). The height mismatch breaks visual rhythm in the button row.
3. **Inline color classes instead of component token** (Low). Hard-coded
   `border-red-300 dark:border-red-700 text-red-600 dark:text-red-400` etc.
   won't update if the red palette changes. Every other button uses
   `.btn-primary` or `.btn-secondary`.

## Suggested fix

Create a `.btn-danger-outline` component class in `input.css`:

```css
.btn-danger-outline {
    @apply border border-red-300 dark:border-red-700 text-red-600
           dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20
           focus:ring-red-500;
}
```

Then apply `btn btn-danger-outline` to the Delete All button, which gives it
the base `.btn` focus ring, padding, and micro-interactions for free.

## WCAG references

- SC 2.4.7 Focus Visible

## Relevant files

- `templates/macros/ui.html` (lines 452-461)
- `static/css/input.css` (lines 82-125, button component classes)
