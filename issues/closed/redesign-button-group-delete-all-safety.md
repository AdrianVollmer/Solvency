# Improve Delete All confirmation UX

## Resolved: XSRF

The XSRF token is already attached to all HTMX requests globally via
`initHtmx()` in `static/ts/main.ts` (the `htmx:configRequest` listener).
No action needed.

## Remaining: native confirm() dialog for nuclear action

`hx-confirm` uses the browser's `window.confirm()` which is not styleable,
looks different across browsers, and provides no friction for an action that
deletes ALL records.

### Suggested fix

- Replace `hx-confirm` with a custom modal (the `#modal-container` in
  `base.html` already exists but is unused).
- The modal should clearly state what will be deleted and how many records
  are affected (e.g., "Delete all 47 transactions?").
- Consider requiring the user to type "DELETE" to confirm, or at minimum
  use a two-button modal where the destructive button is not the default
  focus.
- See also `issues/new/delete-undo-pattern.md` for the broader undo pattern
  proposal, which may supersede this.

## Relevant files

- `templates/macros/ui.html` (`page_action_bar` macro)
- `templates/base.html` (`#modal-container`)
