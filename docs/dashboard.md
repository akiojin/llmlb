# Dashboard

llmlb serves the admin dashboard UI and a lightweight Playground UI.

- Dashboard: `GET /dashboard`
- Playground: `GET /playground`

The authoritative API list and setup instructions live in `README.md` / `README.ja.md`.

## LLM Load Balancer endpoints used by the Dashboard

- `GET /api/dashboard/overview`
- `GET /api/dashboard/stats`
- `GET /api/dashboard/runtimes`
- `GET /api/dashboard/metrics/:runtime_id`
- `GET /api/dashboard/request-history`
- `GET /api/dashboard/request-responses`
- `GET /api/dashboard/request-responses/:id`
- `GET /api/dashboard/request-responses/export`
- `GET /api/dashboard/logs/router`
- `GET /api/runtimes/:runtime_id/logs`
- `GET /api/dashboard/stats/tokens`
- `GET /api/dashboard/stats/tokens/daily`
- `GET /api/dashboard/stats/tokens/monthly`

## Build (regenerate embedded assets)

```bash
pnpm install
pnpm --filter @llmlb/dashboard build
```

This regenerates embedded static assets under `llmlb/src/web/static/`.
