# Improve Delete All safety: XSRF and confirmation UX

The Delete All button performs an irreversible destructive action but has two
safety gaps.

## Problems

1. **XSRF token may not be sent** (High). The button uses `hx-delete` but
   doesn't explicitly include the XSRF token from `<meta name="xsrf-token">`.
   If HTMX isn't globally configured to attach it to DELETE requests, this
   endpoint could be vulnerable to CSRF. Needs verification -- if HTMX is
   already configured globally (e.g., via `htmx.config` or a request header
   hook in main.js), this is a non-issue.
2. **Native confirm() dialog for nuclear action** (Medium). `hx-confirm` uses
   the browser's `window.confirm()` which is not styleable, looks different
   across browsers, and provides no friction for an action that deletes ALL
   records. On mobile, the small touch targets combined with a quick-dismiss
   native dialog make accidental confirmation easy.

## Suggested fixes

### XSRF

- Verify whether `main.js` or HTMX config already attaches the token
  globally. If not, either:
  - Configure HTMX globally:
    `document.body.addEventListener('htmx:configRequest', ...)`
  - Or add `hx-headers` to the button.

### Confirmation UX

- Replace `hx-confirm` with a custom modal (the `#modal-container` in
  `base.html` already exists but is unused).
- The modal should clearly state what will be deleted and how many records
  are affected (e.g., "Delete all 47 transactions?").
- Consider requiring the user to type "DELETE" to confirm, or at minimum
  use a two-button modal where the destructive button is not the default
  focus.
- See also `issues/new/delete-undo-pattern.md` for the broader undo pattern
  proposal.

## Relevant files

- `templates/macros/ui.html` (lines 452-461)
- `templates/base.html` (lines 16, 61)
- `static/js/dist/` (main.js -- check for HTMX global config)
