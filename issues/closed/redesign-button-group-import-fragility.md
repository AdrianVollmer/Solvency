# Fix import button fragility and robustness

Several issues with the Import button and its backing `importResource`
function make it fragile and lacking in user feedback.

## Problems

1. **DOM sibling coupling** (Critical). The Import button uses
   `onclick="this.nextElementSibling.click()"` to trigger a hidden
   `<input type="file">`. Any DOM reorder between the button and input
   silently breaks import. Screen readers also can't infer the relationship.
2. **Hidden file input has no `id`** (Low). The `<input type="file">` has an
   `aria-label` but no `id`, so it can't be referenced by ID from JavaScript.
   Adding an `id` would let the button use
   `document.getElementById('import-file').click()` instead of relying on
   sibling position.
3. **No loading/progress feedback** (Medium). When importing a JSON file,
   there is no visual indication that the operation is in progress. For large
   files the user may think nothing happened and click again, risking
   duplicate imports.
4. **Overly broad querySelector cleanup** (Medium). After import,
   `document.querySelector('input[type="file"]')` clears the first file input
   on the page, not necessarily the one that triggered the import. Currently
   safe (one file input per page) but fragile.

## Suggested fixes

- Give the hidden file input an `id` (e.g., `import-file-input`) and
  reference it by ID from the button.
- Alternatively, wrap both in a `<label>` to use native browser association.
- Pass the input element reference into `importResource` so cleanup targets
  the right element.
- Disable the Import button and show a loading indicator during the async
  operation. Re-enable on completion or error.

## WCAG references

- SC 4.1.2 Name, Role, Value

## Relevant files

- `templates/macros/ui.html` (lines 447-451)
- `templates/base.html` (lines 64-91, `importResource` function)
