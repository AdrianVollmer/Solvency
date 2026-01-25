Add support for XSRF tokens. Ideally, we introduce some sort of
middleware, so all requests that are POST, PUT, DELETE, etc are
discarded unless they have a valid XSRF token.

This means we must endow all forms and HTMX actions with XSRF tokens.
For forms, add a XSRF token in the page's meta data, and have a small piece of
javascript insert it in all forms as a hidden field after the page loaded. Also
set up an observer in case forms get loaded dynamically.
