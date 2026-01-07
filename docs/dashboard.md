# Dashboard

LLM Router serves the admin dashboard UI and a lightweight Playground UI.

- Dashboard: `GET /dashboard`
- Playground: `GET /playground`

The authoritative API list and setup instructions live in `README.md` / `README.ja.md`.

## Router endpoints used by the Dashboard

- `GET /v0/dashboard/overview`
- `GET /v0/dashboard/stats`
- `GET /v0/dashboard/nodes`
- `GET /v0/dashboard/metrics/:node_id`
- `GET /v0/dashboard/request-history`
- `GET /v0/dashboard/request-responses`
- `GET /v0/dashboard/request-responses/:id`
- `GET /v0/dashboard/request-responses/export`
- `GET /v0/dashboard/logs/router`
- `GET /v0/nodes/:node_id/logs`
- `GET /v0/dashboard/stats/tokens`
- `GET /v0/dashboard/stats/tokens/daily`
- `GET /v0/dashboard/stats/tokens/monthly`

## Build (regenerate embedded assets)

```bash
pnpm install
pnpm --filter @llm-router/dashboard build
```

This regenerates embedded static assets under `router/src/web/static/`.
