Add support for a password so unauthenticated users only see the
password prompt.

Add an environment variable that contains a password hashed using
Argon2.

If that variable is set to `DANGEROUSLY_ALLOW_UNAUTHENTICATED_USERS`,
allow unauthenticated users.
