# Stellar Fee Tracker — UI

Next.js (App Router) dashboard for the Stellar Fee Tracker. It shows the current
base/average fee, fee history and congestion trend for the Stellar testnet, and
asks the backend for a recommended fee. It talks to the Rust backend in
[`packages/core`](../core) — it has no data of its own.

## Prerequisites

- Node.js 20 or newer, and npm
- The backend API reachable. Either run it locally (see the
  [root README](../../README.md)) or use `docker compose up` from the repo root.

## Getting started

```bash
npm ci                       # install exactly what package-lock.json pins
cp .env.example .env.local   # change the URLs if the backend is not on :8080
npm run dev                  # http://localhost:3000
```

The landing page is at `/` and the dashboard at `/dashboard`.

## Scripts

| Script | What it does |
|--------|--------------|
| `npm run dev` | Development server with hot reload (Turbopack) |
| `npm run build` | Production build (`next build --turbopack`) |
| `npm run start` | Serve the production build |
| `npm run lint` | ESLint over the project, failing on any warning |
| `npm run typecheck` | `tsc --noEmit` — the only type verification there is |

## Talking to the backend

The dashboard's server component renders `/fees/current`, `/fees/history`,
`/fees/trend` and `/insights` once, then the browser re-polls all four every 10
seconds and on demand via the Refresh button. `/fees/recommend` is a POST issued
from the recommendation panel.

With the recommended `NEXT_PUBLIC_API_URL=/api`, requests go to the same origin
and are proxied to the backend by the rewrite in `next.config.ts`. If you point
`NEXT_PUBLIC_API_URL` at an absolute URL instead, the browser calls the backend
directly and the backend's `ALLOWED_ORIGINS` must include this origin.

## Environment variables

Copy `.env.example` to `.env.local` for local development. Three variables control
how the frontend reaches the Rust backend:

| Variable | Read by | Meaning | Default |
|----------|---------|---------|---------|
| `NEXT_PUBLIC_API_URL` | Browser code (`src/lib/api.ts`) | Base URL for API calls made in the browser. `/api` uses the same-origin Next.js proxy; an absolute URL calls the backend directly (needs CORS via the backend's `ALLOWED_ORIGINS`). Inlined at build time | `http://localhost:8080` |
| `BACKEND_URL` | Server only: `next.config.ts` and server components | Full URL of the backend. Destination of the `/api` rewrite, and the base for server-side fetches. Use `http://backend:8080` under Docker Compose | `http://localhost:8080` |
| `BACKEND_HOST` | Server only: `next.config.ts` and server components | Bare hostname with no protocol (Render supplies it); `https://` is prepended. Wins over `BACKEND_URL` | unset |

### When each applies

- **Browser requests** use `NEXT_PUBLIC_API_URL`. With the recommended `/api` they go to the
  Next.js server, which proxies them to the backend using `BACKEND_HOST` or `BACKEND_URL`.
- **Server-side requests** (the dashboard's server component) cannot use a relative URL,
  because Node has no page origin to resolve it against. They call the backend directly using
  `BACKEND_HOST`, then `BACKEND_URL`, then an absolute `NEXT_PUBLIC_API_URL`.
- **Rewrite destination** (`next.config.ts`) uses `BACKEND_HOST`, then `BACKEND_URL`.

Typical setups:

| Setup | Values |
|-------|--------|
| Local dev | `NEXT_PUBLIC_API_URL=/api`, `BACKEND_URL=http://localhost:8080` |
| Docker Compose | `NEXT_PUBLIC_API_URL=/api`, `BACKEND_URL=http://backend:8080` |
| Render | `NEXT_PUBLIC_API_URL=/api`, `BACKEND_HOST` supplied from the API service |

`NEXT_PUBLIC_STELLAR_NETWORK` also appears in `docker-compose.yml` and `render.yaml`, but the
UI code does not read it yet.

## Docker

`docker compose up frontend` from the repo root builds this package from
`Dockerfile.dev` and mounts the source for hot reload. That image is
**development only** — it runs the dev server, not a production build. For
production, run `npm run build && npm run start` in an image you build yourself.
