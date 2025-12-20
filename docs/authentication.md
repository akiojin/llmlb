# Authentication

LLM Router uses three authentication mechanisms:

1. **JWT** for the admin dashboard and management APIs (`/v0/auth/*`, `/v0/users/*`, `/v0/api-keys/*`, `/v0/dashboard/*`, `/v0/metrics/*`, `/v0/models/*`)
2. **API keys** for OpenAI-compatible endpoints (`/v1/*`) and `/v0` admin/node operations
3. **Node token** for node-to-router heartbeats/metrics (`POST /v0/health`, requires API key too)

The canonical API list lives in `README.md` / `README.ja.md`.

## JWT (admin)

- `POST /v0/auth/login`
- `POST /v0/auth/logout`
- `GET /v0/auth/me`

Clients send the token via `Authorization: Bearer <jwt>`.

## API keys (OpenAI-compatible `/v1/*`)

Protected endpoints (API key + scope required):

- `POST /v1/chat/completions`
- `POST /v1/completions`
- `POST /v1/embeddings`

Model discovery endpoints (API key **or** node token):

- `GET /v1/models`
- `GET /v1/models/:model_id`

Clients send the key via `Authorization: Bearer <api_key>`.

Alternatively, clients can send `X-API-Key: <api_key>`.

### API key scopes

- `api:inference`: OpenAI-compatible `/v1/*` inference endpoints
- `node:register`: `POST /v0/nodes` (node registration), `POST /v0/health` (heartbeat), `GET /v0/models` (node model sync), `GET /v0/models/blob/*` (model blob download)
- `admin:*`: All admin operations (dashboard, users, API keys, model management, metrics, node management)

`admin:*` includes all other scopes. Keys created before scopes were introduced are treated as having all scopes for backward compatibility.

Debug builds only:

- `sk_debug` is accepted for all scopes
- `sk_debug_api` for `api:inference`
- `sk_debug_node` for `node:register`
- `sk_debug_admin` for `admin:*`

## Node token (node → router)

- Router response includes `node_token`
- Node heartbeat/metrics: `POST /v0/health` with `X-Node-Token: <node_token>` + API key (`Authorization: Bearer <api_key>`)
- `GET /v1/models` can also be called with `X-Node-Token` (OpenAI-compatible list)
