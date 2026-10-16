This is a [Next.js](https://nextjs.org) project bootstrapped with [`create-next-app`](https://nextjs.org/docs/app/api-reference/cli/create-next-app).

## Getting Started

First, run the development server:

```bash
npm run dev
# or
yarn dev
# or
pnpm dev
# or
bun dev
```

Open [http://localhost:3000](http://localhost:3000) with your browser to see the result.

You can start editing the page by modifying `app/page.tsx`. The page auto-updates as you edit the file.

This project uses [`next/font`](https://nextjs.org/docs/app/building-your-application/optimizing/fonts) to automatically optimize and load [Geist](https://vercel.com/font), a new font family for Vercel.

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

## Learn More

To learn more about Next.js, take a look at the following resources:

- [Next.js Documentation](https://nextjs.org/docs) - learn about Next.js features and API.
- [Learn Next.js](https://nextjs.org/learn) - an interactive Next.js tutorial.

You can check out [the Next.js GitHub repository](https://github.com/vercel/next.js) - your feedback and contributions are welcome!

## Deploy on Vercel

The easiest way to deploy your Next.js app is to use the [Vercel Platform](https://vercel.com/new?utm_medium=default-template&filter=next.js&utm_source=create-next-app&utm_campaign=create-next-app-readme) from the creators of Next.js.

Check out our [Next.js deployment documentation](https://nextjs.org/docs/app/building-your-application/deploying) for more details.
