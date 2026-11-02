# Authentication

llmlb always requires authentication. There is no anonymous / no-auth mode: protected APIs reject
unauthenticated requests with `401 Unauthorized`. On first boot llmlb provisions an admin account
(from `ADMIN_PASSWORD` or an interactive prompt; debug builds also accept `admin` / `test`).

llmlb uses two authentication mechanisms:

1. **JWT** for the dashboard and management APIs (`/api/*`).
   - `/api/dashboard/*` is **JWT-only** (API keys are rejected).
2. **API keys** for the OpenAI-compatible API (`/v1/*`) and selected `/api/*` endpoints for
   ops automation.

The canonical API list lives in `README.md` / `README.ja.md`.
Requirements, per-user-story implementation status, and remaining tasks are tracked in
[SPEC #580](https://github.com/akiojin/llmlb/issues/580).

## JWT (dashboard + management)

- `POST /api/auth/login`
- `POST /api/auth/logout`
- `GET /api/auth/me`
- `PUT /api/auth/change-password` (requires `current_password`)

### Password reset

- `POST /api/auth/forgot-password` (no auth) issues a reset token for an email ID. It always
  returns `202` and never includes the token, so account existence is not revealed.
- There is no mail delivery. The reset link `/dashboard/reset-password.html#token=...` is written
  to the server log; the operator hands it to the user.
- `POST /api/auth/reset-password` (no auth) consumes the token and sets `new_password`. Tokens
  expire after 30 minutes, are single-use, and only the latest token per user is valid.
- Only the SHA-256 hash of a token is stored. A successful reset or password change bumps
  `password_changed_at`, which revokes existing sessions.

Accepted transports:

- `Authorization: Bearer <jwt>`
- HttpOnly cookie `llmlb_jwt` (used by the dashboard)

### CSRF (cookie auth only)

When authenticating via cookies and making mutating requests (POST/PUT/PATCH/DELETE):

- send `X-CSRF-Token: <token>` where `<token>` is the value of the `llmlb_csrf` cookie
- `Origin`/`Referer` must match the dashboard origin

If you authenticate via `Authorization` header, CSRF checks are not applied.

## API keys

Clients send the key via:

- `Authorization: Bearer <api_key>`
- `X-API-Key: <api_key>`

### Permissions

API keys carry a list of `permissions` (string IDs). The backend default-denies: keys without
permissions cannot access protected endpoints. The legacy `scopes` field is deprecated and is
rejected by the API.

| Permission | Grants |
|---|---|
| `openai.inference` | OpenAI-compatible inference endpoints (`POST /v1/*` except `GET /v1/models*`) |
| `openai.models.read` | Model discovery (`GET /v1/models`, `GET /v1/models/:model_id`) |
| `endpoints.read` | Read-only endpoint APIs (`GET /api/endpoints*`) |
| `endpoints.manage` | Endpoint mutations (`POST/PUT/DELETE /api/endpoints*`, `POST /api/endpoints/:id/test`, `POST /api/endpoints/:id/sync`, `POST /api/endpoints/:id/download`) |
| `users.manage` | User and registration request management (`/api/users*`, `/api/registration-requests*`) |
| `models.manage` | Model register/delete (`POST /api/models/register`, `DELETE /api/models/*`) |
| `registry.read` | Model registry access (`GET /api/models/registry/*`) and model list endpoints (`GET /api/models`, `GET /api/models/hub`) |
| `logs.read` | Node log proxy (`GET /api/nodes/:node_id/logs`) |
| `metrics.read` | Metrics export (`GET /api/metrics/cloud`, `GET /api/metrics/cloud/export`) |

### Self-service API key management

Authenticated users list keys within their current scope using JWT-only endpoints.
Updates and deletion still require issuer ownership:

- `GET /api/me/api-keys`
- `POST /api/me/api-keys`
- `PUT /api/me/api-keys/:id`
- `DELETE /api/me/api-keys/:id`

`POST /api/me/api-keys` creates an inference key for an active organization
account. System administrators specify a non-empty subset of
`openai.inference` and `openai.models.read`; members omit `permissions` and
receive both. New attributed keys cannot receive or use global automation
permissions. Existing unattributed operational keys retain their explicit
legacy permission gates.

Note: For routes that accept either JWT or API key, llmlb prefers JWT if present. Avoid sending a
JWT-looking token (three dot-separated segments) as an API key in `Authorization: Bearer`.

### Debug keys (debug builds only)

In debug builds (`#[cfg(debug_assertions)]`) the following API keys are accepted:

- `sk_debug`: all permissions
- `sk_debug_admin`: all permissions
- `sk_debug_api`: `openai.inference` + `openai.models.read`
- `sk_debug_runtime`: `registry.read`

## Registration approval

Public registration (`POST /api/auth/register`) accepts an email-format `username` and
`password`, returning `202` with `status: "pending"`. Passwords are hashed in a separate
request table; no user or authentication token exists before approval. Pending and
rejected applicants receive the same login error as nonexistent users.

Administrators review pending requests in Manage Users or use
`GET /api/registration-requests`. Approve via
`POST /api/registration-requests/:id/approve` with an explicit `role` (`admin` or `viewer`),
or reject via `DELETE /api/registration-requests/:id`. These endpoints also accept API
keys with `users.manage`. Rejection permits reapplication. The dashboard defaults to
`viewer`; later role changes remain available, with protection for the last admin.

Pending requests are limited to 100, and usernames are unique across users and requests.
New requests use the existing administrator notification settings and email addresses.
Missing mail configuration or a delivery failure does not reject a registration.
Invitation APIs, permissions, and all unused invitation codes were removed in 6.4.0.

## Organizations and usage attribution

A shared llmlb URL can serve multiple organizations. System `admin` remains a
separate role from organization `admin` / `member`; an organization administrator
never gains system administration permissions. Each account has at most one
organization. An unassigned or disabled non-administrator cannot run inference or
read usage. JWT requests use the current account and membership in SQLite,
including when a token was issued before a transfer or role change.

Usage, request lists, details, exports, pagination totals and token counters are
restricted to the authenticated scope:

| Account | Read scope | Organization and membership changes |
| --- | --- | --- |
| System administrator | All organizations and unattributed records | Allowed |
| Organization administrator | Current organization | Denied |
| Member | Own account within the current organization | Denied |
| Unassigned or disabled account | No usage access | Denied |
| API key | No dashboard usage access | Denied |

The dashboard **Organizations** dialog lets a system administrator create, rename
and archive empty organizations, assign accounts, and set organization roles.
Cookie mutations require the same CSRF protection as existing management APIs.
Archiving an organization with assigned accounts is refused.

New inference keys carry a usage account and organization independently of the
issuing administrator (`created_by`). System administrators can issue an
explicitly attributed inference key from an organization's account list. A key
cannot choose its organization through request headers, body fields or query
parameters. Transferring, unassigning or disabling its account revokes the old
organization key; moving back does not reactivate it. Issue a new key for the
new membership. Historical usage stays with the account and organization
recorded when the request was received.

### Upgrade from existing accounts and keys

1. Upgrade with a backup of the SQLite database. Migrations preserve system
   administrators, existing keys, request history and audit records.
2. Create organizations and assign accounts using a system administrator.
   Existing accounts are not silently assigned to a presumed historical tenant.
3. Issue new keys with explicit usage attribution. Distribute them through your
   existing credential process. The full key is displayed only at creation.
4. Revoke an old key explicitly after clients have moved. Old keys continue
   inference as unattributed credentials until revoked; their issuer is not
   treated as proof of who used them.

The old `viewer` ability to read global dashboard usage is intentionally
replaced by organization or own-account access. Existing raw history with unknown
attribution is visible only to system administrators. Past data that has already
expired is not reconstructed or reassigned to current memberships.
