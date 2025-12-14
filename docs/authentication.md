# Authentication

LLM Router uses three authentication mechanisms:

1. **JWT** for the admin dashboard and management APIs (`/v0/auth/*`, `/v0/users/*`, `/v0/api-keys/*`)
2. **API keys** for OpenAI-compatible endpoints (`/v1/*`)
3. **Node token** for node-to-router heartbeats/metrics (`POST /v0/health`)

The canonical API list lives in `README.md` / `README.ja.md`.

## JWT (admin)

- `POST /v0/auth/login`
- `POST /v0/auth/logout`
- `GET /v0/auth/me`

Clients send the token via `Authorization: Bearer <jwt>`.

## API keys (OpenAI-compatible `/v1/*`)

Protected endpoints (API key required):

- `POST /v1/chat/completions`
- `POST /v1/completions`
- `POST /v1/embeddings`

Model discovery endpoints (API key **or** node token):

- `GET /v1/models`
- `GET /v1/models/:model_id`

Clients send the key via `Authorization: Bearer <api_key>`.

Alternatively, clients can send `X-API-Key: <api_key>`.

Debug builds only:

- A fixed API key `sk_debug` is accepted for `/v1/*` to simplify local development.

## Node token (node → router)

- Node registration: `POST /v0/nodes` (GPU is required)
- Router response includes `node_token`
- Node model sync: `GET /v1/models` with `X-Node-Token: <node_token>`
- Node heartbeat/metrics: `POST /v0/health` with `X-Node-Token: <node_token>`
