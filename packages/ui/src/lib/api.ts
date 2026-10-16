import type {
  CurrentFeeResponse,
  FeeHistoryResponse,
  FeeTrendResponse,
  InsightsResponse,
  RecommendResponse,
  RecommendRequest,
} from './types'

const DEFAULT_BACKEND_URL = 'http://localhost:8080'

/**
 * Where API requests are sent.
 *
 * In the browser a relative base such as `/api` is fine: it is resolved against the
 * page origin and handled by the Next.js rewrite in `next.config.ts`.
 *
 * On the server (server components such as `dashboard/page.tsx`) there is no page
 * origin, so `fetch('/api/...')` cannot be parsed and rejects. The server must call
 * the backend directly with an absolute URL, resolved the same way `next.config.ts`
 * resolves the rewrite destination: `BACKEND_HOST`, then `BACKEND_URL`.
 */
export function resolveApiBase(
  isServer: boolean = typeof window === 'undefined',
  // Each variable is read as a literal `process.env.NAME` because Next.js only
  // inlines NEXT_PUBLIC_* values into the browser bundle for that exact form.
  env: Record<string, string | undefined> = {
    NEXT_PUBLIC_API_URL: process.env.NEXT_PUBLIC_API_URL,
    BACKEND_HOST: process.env.BACKEND_HOST,
    BACKEND_URL: process.env.BACKEND_URL,
  },
): string {
  const publicUrl = env.NEXT_PUBLIC_API_URL

  if (!isServer) {
    return publicUrl || DEFAULT_BACKEND_URL
  }

  if (env.BACKEND_HOST) return `https://${env.BACKEND_HOST}`
  if (env.BACKEND_URL) return env.BACKEND_URL
  // An absolute NEXT_PUBLIC_API_URL is already usable on the server as-is.
  if (publicUrl && /^https?:\/\//i.test(publicUrl)) return publicUrl
  return DEFAULT_BACKEND_URL
}

async function post<T>(path: string, body: unknown): Promise<T> {
  const res = await fetch(`${resolveApiBase()}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: res.statusText }))
    throw new Error(err.error || `API error ${res.status} on ${path}`)
  }
  return res.json() as Promise<T>
}

async function get<T>(path: string): Promise<T> {
  const res = await fetch(`${resolveApiBase()}${path}`, {
    next: { revalidate: 0 }, // always fresh
  })
  if (!res.ok) {
    throw new Error(`API error ${res.status} on ${path}`)
  }
  return res.json() as Promise<T>
}

async function getText(path: string): Promise<string> {
  const res = await fetch(`${BASE}${path}`, {
    next: { revalidate: 0 }, // always fresh
  })
  if (!res.ok) {
    throw new Error(`API error ${res.status} on ${path}`)
  }
  return res.text()
}

export const api = {
  currentFees:  ()               => get<CurrentFeeResponse>('/fees/current'),
  feeHistory:   (window = '1h')  => get<FeeHistoryResponse>(`/fees/history?window=${window}`),
  feeTrend:     ()               => get<FeeTrendResponse>('/fees/trend'),
  insights:     ()               => get<InsightsResponse>('/insights'),
  // /health returns the plain-text body "ok", not JSON.
  health:       async ()         => (await getText('/health')).trim() === 'ok',
  recommend:    (body: RecommendRequest) => post<RecommendResponse>('/fees/recommend', body),
}
