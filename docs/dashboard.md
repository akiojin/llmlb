# Dashboard

LLM Router serves the admin dashboard UI and a lightweight Playground UI.

- Dashboard: `GET /dashboard`
- Playground: `GET /playground`

The authoritative API list and setup instructions live in `README.md` / `README.ja.md`.

## Router endpoints used by the Dashboard

- `GET /api/dashboard/overview`
- `GET /api/dashboard/stats`
- `GET /api/dashboard/nodes`
- `GET /api/dashboard/metrics/:node_id`
- `GET /api/dashboard/request-history`
- `GET /api/dashboard/request-responses`
- `GET /api/dashboard/request-responses/:id`
- `GET /api/dashboard/request-responses/export`
- `GET /api/dashboard/logs/router`
- `GET /api/nodes/:node_id/logs`

## Build (regenerate embedded assets)

```bash
pnpm install
pnpm --filter @llm-router/dashboard build
```

This regenerates embedded static assets under `router/src/web/static/`.
