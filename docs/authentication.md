# Authentication

LLM Router uses three authentication mechanisms:

1. **JWT** for the admin dashboard and management APIs (`/api/auth/*`, `/api/users/*`, `/api/api-keys/*`)
2. **API keys** for OpenAI-compatible endpoints (`/v1/*`)
3. **Agent token** for node-to-router heartbeats/metrics (`POST /api/health`)

The canonical API list lives in `README.md` / `README.ja.md`.

## JWT (admin)

- `POST /api/auth/login`
- `POST /api/auth/logout`
- `GET /api/auth/me`

Clients send the token via `Authorization: Bearer <jwt>`.

## API keys (OpenAI-compatible `/v1/*`)

Protected endpoints (API key required):

- `POST /v1/chat/completions`
- `POST /v1/completions`
- `POST /v1/embeddings`

Model discovery endpoints (API key **or** agent token):

- `GET /v1/models`
- `GET /v1/models/:model_id`

Clients send the key via `Authorization: Bearer <api_key>`.

Alternatively, clients can send `X-API-Key: <api_key>`.

Debug builds only:

- A fixed API key `sk_debug` is accepted for `/v1/*` to simplify local development.

## Agent token (node → router)

- Node registration: `POST /api/nodes` (GPU is required)
- Router response includes `agent_token`
- Node model sync: `GET /v1/models` with `X-Agent-Token: <agent_token>`
- Node heartbeat/metrics: `POST /api/health` with `X-Agent-Token: <agent_token>`
