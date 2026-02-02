## Session cookie missing Secure flag; no login rate limiting

**Priority: Security (High)**

### Missing Secure flag

`src/auth.rs:120-125` sets the session cookie without the `Secure`
flag:

```rust
let cookie = Cookie::build((SESSION_COOKIE, session_token))
    .path("/")
    .http_only(true)
    .same_site(SameSite::Strict)
    .build();
```

The cookie is sent over plaintext HTTP, making it vulnerable to
interception via MITM.

### No rate limiting on /login

`src/auth.rs:106-143` (`login_submit`) has no throttling. An attacker
can make unlimited password guessing attempts. While Argon2 is
intentionally slow, this also means each attempt consumes significant
CPU, making the login endpoint a vector for CPU-based DoS.

### Fix

- Add `.secure(true)` to the cookie builder (or make it conditional
  on a config flag for local development).
- Add rate limiting on `/login`, e.g. exponential backoff after N
  failed attempts from the same IP. A simple in-memory counter with
  a cooldown is sufficient.
