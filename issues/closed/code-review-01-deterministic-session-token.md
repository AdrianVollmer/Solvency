## Session token is deterministic, not random

**Priority: Security (Critical)**

`generate_session_token` in `src/auth.rs:172-179` produces the same
token for every login by hashing the password hash with a fixed prefix
using `DefaultHasher` (SipHash, 64-bit output):

```rust
fn generate_session_token(password_hash: &str) -> String {
    let mut hasher = DefaultHasher::new();
    "solvency_session_v1".hash(&mut hasher);
    password_hash.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
```

Problems:

1. All sessions share the same token. Leaking one (logs, XSS,
   network sniff) compromises all sessions permanently.
2. An attacker who obtains the Argon2 hash (e.g. from `.env`) can
   compute the session token offline without knowing the password.
3. 64-bit SipHash is too short -- feasible to brute-force.

### Fix

Generate a cryptographically random token per session (e.g.
`rand::thread_rng().gen::<[u8; 32]>()` or `uuid::Uuid::new_v4()`).
Store valid tokens server-side in an in-memory set (or a simple
HashMap with expiry). Invalidate individual sessions on logout.
