# Dashboard

llmlb serves the admin dashboard UI as a React SPA.

- Dashboard shell: `GET /dashboard`
- Dashboard static assets: `GET /dashboard/*`
- Endpoint Playground route: `/dashboard/#playground/:endpointId`
- LB Playground route: `/dashboard/#lb-playground`

## Playground modes

### Endpoint Playground

- Route: `#playground/:endpointId`
- Purpose: direct endpoint verification
- API: `POST /api/endpoints/:id/chat/completions` (JWT only)

### LB Playground

- Route: `#lb-playground`
- Purpose: load balancer routing and distribution validation
- Modes:
  - Chat: available to all authenticated users.
  - Load Test: **admin role only**. It issues many real inference
    requests through the load balancer, so viewers cannot run it
    (hidden in the UI and rejected with 403 by the backend).
- APIs:
  - `GET /api/dashboard/playground/models` (JWT/session)
  - `POST /api/dashboard/playground/chat/completions`
    (Chat; JWT/session + CSRF)
  - `POST /api/dashboard/playground/load-test/chat/completions`
    (Load Test; JWT/session + CSRF + admin role)
  - `GET /api/dashboard/request-responses`
    (JWT only, for distribution aggregation)

## Operational notifications (admin only)

Both dialogs are opened from the user menu in the header.

- **Manage Users**: each user has an optional notification email, shown in
  the list and editable in the create / edit dialogs. It is the destination
  for operational notifications, not a login identifier. Clearing the field
  removes the address.
- **Notifications**: turns the daily digest on or off and edits the send
  time, the email language and the non-secret SMTP settings (host, port, from
  address). It lists the recipients (administrators with a notification
  email) and shows the status. When notifications are enabled but cannot be
  sent, the reason is shown in the dialog and repeated when saving. The SMTP
  credentials are set in the server environment only; the dialog shows
  whether they are set.
- APIs: `GET` / `PUT /api/dashboard/notifications` (JWT/session + CSRF + admin
  role), `POST /api/users`, `PUT /api/users/:id`. See
  [API.md](API.md) for the settings and the delivery behaviour.

## Dashboard APIs used by UI

- `GET /api/dashboard/overview`
- `GET /api/dashboard/stats`
- `GET /api/dashboard/endpoints`
- `GET /api/dashboard/metrics/:node_id`
- `GET /api/dashboard/request-history`
- `GET /api/dashboard/request-responses`
- `GET /api/dashboard/request-responses/:id`
- `GET /api/dashboard/request-responses/export`
- `GET /api/dashboard/logs/lb`
- `GET /api/dashboard/stats/tokens`
- `GET /api/dashboard/stats/tokens/daily`
- `GET /api/dashboard/stats/tokens/monthly`

## Tests: which layer verifies what

The dashboard is verified by three layers. Put a check in the cheapest layer
that can observe the behaviour; do not write the same check in two layers.

| Layer | Location | Command | Needs |
|-------|----------|---------|-------|
| Component tests (vitest + React Testing Library) | `llmlb/src/web/dashboard/src/**/*.test.ts(x)` | `pnpm --filter @llm/dashboard test` | Node only |
| Source and artifact checks (Rust) | `llmlb/tests/ui/*.rs` | `cargo test --test ui_tests` | Rust toolchain |
| End-to-end tests (Playwright) | `llmlb/tests/e2e-playwright/specs/` | `make e2e-playwright` | Rust toolchain and a browser (Playwright starts the server) |

`make dashboard-checks` runs typecheck, lint and the component tests. The
`Dashboard Typecheck, Lint & Test` job in `.github/workflows/lint.yml` runs it
on every pull request to `develop`, without a path filter.

### Component tests

Use them for anything a rendered component decides on its own:

- what is rendered from given data, and the loading / empty / error states
- which API method a user action calls, and with which arguments
- role gating inside a page (what an admin sees and a viewer does not)
- hook logic such as WebSocket query invalidation. Each ViewModel declares its
  dependencies with `src/hooks/useInvalidateOn.ts`; the subscription registry in
  `src/hooks/dashboardSubscriptions.ts` invalidates matching query keys when
  `useWebSocket` receives `{changed, id?}`. An id-less notification reaches all
  subscribers of the resource; an id-scoped notification reaches aggregate
  subscribers and subscribers declaring that same id. Connection state comes
  from `onopen`.
- resource type and subscription coverage. `DashboardResource` is a union derived
  from the single `DASHBOARD_RESOURCES` constant in `src/lib/dashboardResources.ts`;
  unknown resource names fail typecheck in `dashboardResources.type-test.ts`.
  `src/viewmodels/dashboardResourceCoverage.test.tsx` requires a behavior row for
  every resource and verifies that mounted production ViewModels subscribe and
  invalidate the expected query keys. CI runs these checks through
  `make dashboard-checks`, and directly runs `make dashboard-data-hooks` to reject
  data-fetching hooks in views.

They run in jsdom without a server. `src/test/setup.ts` replaces `fetch` and
`WebSocket`; a request that a test did not stub fails that test. Stub the API
objects from `@/lib/api` with `vi.spyOn`, and render through
`renderWithProviders` / `renderPage` from `src/test/render.tsx`.

Do not use them for:

- full-page navigation (`window.location.href = ...`): jsdom does not navigate
- layout, theme and chart rendering: jsdom has no layout engine
- the contract with the real backend

### Source and artifact checks

`llmlb/tests/ui/*.rs` reads source text (`include_str!`) or the HTML that the
server returns. Use them only for properties of the source or of the embedded
bundle that no rendered test can observe:

- a removed feature stays removed (an identifier or string must not exist)
- the stylesheet defines the theme tokens and the reduced-motion rule
- the served HTML shell has the mount point and no leftover markup
- a defensive guard that the UI never reaches (for example the admin check
  inside `startLoadTest`, whose trigger is hidden from viewers)

Do not add a source-text assertion for behaviour (rendering, events, state
changes). A string being present in a `.tsx` file does not show that it is
rendered. When a component test covers what a source-text check asserts,
delete the source-text check in the same change.

### End-to-end tests

Use Playwright for what only a real browser and server can show:

- flows across pages and redirects (login, forced password change, logout)
- the real API contract, authentication and CSRF
- live updates over the real WebSocket
- themes, layout, charts and screenshots

Screen coverage has one source: the canonical screen registry in
`llmlb/tests/e2e-playwright/helpers/screen-registry.ts`. Each entry names a
screen (page, hash route, dashboard tab or modal) and knows how to reach it.
The navigation, back-navigation and screenshot suites derive their cases from
it, so when you add or remove a screen, change the registry entry and nothing
else. `specs/dashboard/screen-registry.spec.ts` fails if a suite stops
following the registry.

## Build (regenerate embedded assets)

```bash
pnpm install
pnpm --filter @llm/dashboard build
```

This regenerates embedded static assets under `llmlb/src/web/static/`.
