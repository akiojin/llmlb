# Authentication

LLM Router uses three authentication mechanisms:

1. **JWT** for the admin dashboard and management APIs (`/v0/auth/*`, `/v0/users/*`, `/v0/api-keys/*`, `/v0/dashboard/*`, `/v0/metrics/*`, `/v0/models/*`)
2. **API keys** for OpenAI-compatible endpoints (`/v1/*`) and `/v0` admin/runtime operations
3. **Runtime token** for runtime-to-router heartbeats/metrics (`POST /v0/health`, requires API key too)

The canonical API list lives in `README.md` / `README.ja.md`.

## JWT (admin)

- `POST /v0/auth/login`
- `POST /v0/auth/logout`
- `GET /v0/auth/me`

Clients send the token via `Authorization: Bearer <jwt>`.

## API keys (OpenAI-compatible `/v1/*`)

Protected endpoints (API key with `api` scope required, Responses API recommended):

- `POST /v1/responses`
- `POST /v1/chat/completions`
- `POST /v1/completions`
- `POST /v1/embeddings`

Model discovery endpoints (API key **or** runtime token):

- `GET /v1/models`
- `GET /v1/models/:model_id`

Clients send the key via `Authorization: Bearer <api_key>`.

Alternatively, clients can send `X-API-Key: <api_key>`.

Note: If `Authorization: Bearer` looks like a JWT (three dot-separated segments) and JWT
verification fails, the request is rejected and API key fallback is not attempted. Clients
should send only one auth scheme per request.

### API key scopes

- `api`: OpenAI-compatible `/v1/*` inference endpoints
- `runtime`: `POST /v0/runtimes` (runtime registration), `POST /v0/health` (heartbeat), `GET /v0/models` (runtime model sync), `GET /v0/models/registry/:model_name/manifest.json` (manifest)
- `admin`: All admin operations (dashboard, users, API keys, model management, metrics, runtime management)

`admin` includes all other scopes. Keys created before scopes were introduced are treated as having all scopes for backward compatibility.

Debug builds only:

- `sk_debug` is accepted for all scopes
- `sk_debug_api` for `api`
- `sk_debug_runtime` for `runtime`
- `sk_debug_admin` for `admin`

## Runtime token (runtime → router)

- Router response includes `runtime_token`
- Runtime heartbeat/metrics: `POST /v0/health` with `X-Runtime-Token: <runtime_token>` + API key (`Authorization: Bearer <api_key>`)
- `GET /v1/models` can also be called with `X-Runtime-Token` (OpenAI-compatible list)
