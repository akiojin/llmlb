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

Protected endpoints:

- `POST /v1/chat/completions`
- `POST /v1/completions`
- `POST /v1/embeddings`
- `GET /v1/models`
- `GET /v1/models/:model_id`

Clients send the key via `Authorization: Bearer <api_key>`.

Dev-only bypass:

- `LLM_ROUTER_SKIP_API_KEY=1`

## Agent token (node → router)

- Node registration: `POST /api/nodes` (GPU is required)
- Router response includes `agent_token`
- Node heartbeat/metrics: `POST /api/health` with `X-Agent-Token: <agent_token>`
