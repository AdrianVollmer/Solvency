# Minor UX improvements to action bar

Two smaller issues that don't warrant individual tracking.

## 1. Replace window.location.reload() after Delete All (Medium)

After a successful Delete All, the `hx-on::after-request` handler calls
`window.location.reload()`. This causes a full page flash -- the sidebar,
navbar, and all page content re-render. The user loses scroll position.

**Fix:** Use HTMX to swap only the content area. For example, trigger a
targeted `hx-get` on the table/list container after deletion succeeds, or
use `hx-target` on the button itself to replace the page content region.

## 2. Export link doesn't indicate file download (Low)

The Export `<a>` tag has no `download` attribute and no indication that
clicking it downloads a file rather than navigating. Screen readers don't
announce it as a download action.

**Fix:** Add the `download` attribute to the `<a>` tag and optionally
append a format hint to the label (e.g., "Export JSON") or add
`aria-label="Export as JSON file"`.

## Relevant files

- `templates/macros/ui.html` (lines 443-445 for Export, line 457 for reload)
