# Minor UX improvements to action bar

## 1. Export link now indicates file download (Low) -- Fixed

Added `download` attribute to Export `<a>` tags in both desktop inline and
mobile dropdown menu views.

## 2. window.location.reload() after Delete All (Medium) -- Won't fix

After a successful Delete All, the page does a full reload. While an HTMX
partial swap would avoid the flash, each manage page has different content
structure, and "delete all" empties everything anyway -- a full reload showing
the empty state is the simplest correct behavior. The reload is brief and
acceptable UX for a rare destructive action.
