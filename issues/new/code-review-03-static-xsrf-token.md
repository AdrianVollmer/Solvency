## XSRF token is static for the process lifetime

**Priority: Security (High)**

A single XSRF token is generated at server startup (`src/server.rs:36`)
and reused for every request, every user, forever:

```rust
let xsrf_token = XsrfToken::generate();  // once, at startup
```

Since the token is embedded in every HTML page's `<meta>` tag, anyone
who can view-source on any page has the XSRF token. Combined with the
deterministic session token (issue #01), this means both auth
credentials are fixed values.

### Fix

Generate per-session XSRF tokens. Bind the XSRF token to the session
cookie (e.g. HMAC of session token with a server secret). Rotate on
each session.
