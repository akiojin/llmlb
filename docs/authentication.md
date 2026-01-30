# Authentication

LLM Router uses three authentication mechanisms:

1. **JWT** for the admin dashboard and management APIs (`/api/auth/*`, `/api/users/*`, `/api/api-keys/*`, `/api/dashboard/*`, `/api/metrics/*`, `/api/models/*`)
2. **API keys** for OpenAI-compatible endpoints (`/v1/*`) and `/api` admin/runtime operations
3. **Runtime token** for runtime-to-router heartbeats/metrics (`POST /api/health`, requires API key too)

The canonical API list lives in `README.md` / `README.ja.md`.

## JWT (admin)

- `POST /api/auth/login`
- `POST /api/auth/logout`
- `GET /api/auth/me`

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
- `runtime`: `POST /api/runtimes` (runtime registration), `POST /api/health` (heartbeat), `GET /api/models` (runtime model sync), `GET /api/models/registry/:model_name/manifest.json` (manifest)
- `admin`: All admin operations (dashboard, users, API keys, model management, metrics, runtime management)

`admin` includes all other scopes. Keys created before scopes were introduced are treated as having all scopes for backward compatibility.

Debug builds only:

- `sk_debug` is accepted for all scopes
- `sk_debug_api` for `api`
- `sk_debug_runtime` for `runtime`
- `sk_debug_admin` for `admin`

## Runtime token (runtime → router)

- Router response includes `runtime_token`
- Runtime heartbeat/metrics: `POST /api/health` with `X-Runtime-Token: <runtime_token>` + API key (`Authorization: Bearer <api_key>`)
- `GET /v1/models` can also be called with `X-Runtime-Token` (OpenAI-compatible list)
