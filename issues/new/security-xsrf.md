Add support for XSRF tokens. Ideally, we introduce some sort of
middleware, so all requests that are POST, PUT, DELETE, etc are
discarded unless they have a valid XSRF token.

This means we must endow all forms and HTMX actions with XSRF tokens.
Ideally, we find a way that all forms automatically receive a hidden
field with the XSRF token.
