Add support for a password so unauthenticated users only see the password
prompt.

Add an environment variable that contains a password hashed using Argon2.

If that variable isn't present, allow unauthenticated users.
