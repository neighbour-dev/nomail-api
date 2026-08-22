# NoMail API

## Single-user account

Public registration is disabled. On the first startup, when the `users` table
is empty, configure the initial account in `.env`:

```dotenv
INITIAL_USERNAME=admin
INITIAL_PASSWORD=change-me-now1
```

The password must contain at least 8 characters. After the account is created,
remove `INITIAL_PASSWORD` from the environment. Existing accounts are never
overwritten by these variables.

The current account can be queried with `GET /api/auth/account` using a Bearer
access token.

An authenticated user can update the account with:

```http
PATCH /api/auth/account
Authorization: Bearer <access-token>
Content-Type: application/json

{
  "currentPassword": "current-password",
  "username": "new-username",
  "newPassword": "new-password"
}
```

`username` and `newPassword` are optional individually, but at least one is
required. Changing the password revokes all refresh tokens.

If the credentials are lost, run the reset command on the server:

```bash
go run ./cmd/reset-account --username admin --password new-password
```

The username flag can be omitted to preserve the current username. Resetting
the account password revokes all refresh tokens.
