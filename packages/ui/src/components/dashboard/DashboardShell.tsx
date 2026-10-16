'use client'

import { useState, useEffect, useCallback, useRef } from 'react'
import { api } from '@/lib/api'
import type {
  CurrentFeeResponse,
  FeeHistoryResponse,
  FeeTrendResponse,
  InsightsResponse,
} from '@/lib/types'
import { TopBar }             from './TopBar'
import { StatCards }          from './StatCards'
import { PercentileRow }      from './PercentileRow'
import { RecommendationPanel } from './RecommendationPanel'
import { FeeChart }           from './FeeChart'
import { TrendPanel }         from './TrendPanel'
import { RollingAverages }    from './RollingAverages'

interface DashboardData {
  current:  CurrentFeeResponse  | null
  history:  FeeHistoryResponse  | null
  trend:    FeeTrendResponse    | null
  insights: InsightsResponse    | null
  error:    string | null
}

interface Props {
  initialData: DashboardData
}

type RequestKey = 'current' | 'history' | 'trend' | 'insights'

const POLL_MS = 10_000

export function DashboardShell({ initialData }: Props) {
  const [data,        setData]        = useState<DashboardData>(initialData)
  const [window,      setWindow]      = useState<'1h' | '6h' | '24h'>('1h')
  const [lastUpdated, setLastUpdated] = useState<Date>(new Date())
  const [isRefreshing, setIsRefreshing] = useState(false)
  // Which endpoints failed on the most recent poll. A null slice with
  // `failed[key]` set is an error to report, not a request still in flight.
  const [failed, setFailed] = useState<Partial<Record<RequestKey, boolean>>>({})
  // The chart window lives in a ref as well as state so `refresh` can read the
  // current value without depending on it. That keeps `refresh` stable, so the
  // polling interval below is created once instead of after every response.
  const windowRef = useRef<'1h' | '6h' | '24h'>('1h')
  const controllerRef = useRef<AbortController | null>(null)

  const refresh = useCallback(async (win?: '1h' | '6h' | '24h') => {
    // Cancel any request still in flight: a newer refresh (including one for a
    // different window) supersedes it, so responses cannot overlap or land out of order.
    controllerRef.current?.abort()
    const controller = new AbortController()
    controllerRef.current = controller
    const { signal } = controller

    setIsRefreshing(true)
    const [current, history, trend, insights] = await Promise.allSettled([
      api.currentFees(signal),
      api.feeHistory(win ?? windowRef.current, signal),
      api.feeTrend(signal),
      api.insights(signal),
    ])

    // Superseded by a newer refresh, or the component unmounted: ignore the result.
    if (signal.aborted) return

    // Functional update, so the stale-data fallback uses the latest state rather
    // than a captured copy (which would force `refresh` to depend on `data`).
    setData(prev => ({
      current:  current.status  === 'fulfilled' ? current.value  : prev.current,
      history:  history.status  === 'fulfilled' ? history.value  : prev.history,
      trend:    trend.status    === 'fulfilled' ? trend.value    : prev.trend,
      insights: insights.status === 'fulfilled' ? insights.value : prev.insights,
      error:    null,
    }))
    setFailed({
      current:  current.status  === 'rejected',
      history:  history.status  === 'rejected',
      trend:    trend.status    === 'rejected',
      insights: insights.status === 'rejected',
    })
    setLastUpdated(new Date())
    setIsRefreshing(false)
  }, [])

  // Auto-poll every 10s. `refresh` is stable, so this runs once; on unmount the
  // interval is cleared and any request still in flight is aborted.
  useEffect(() => {
    const id = setInterval(() => refresh(), POLL_MS)
    return () => {
      clearInterval(id)
      controllerRef.current?.abort()
    }
  }, [refresh])

  // Re-fetch history when window changes
  const handleWindowChange = async (w: '1h' | '6h' | '24h') => {
    windowRef.current = w
    setWindow(w)
    refresh(w)
  }

  const { current, history, trend, insights } = data

  // Fetch health for the TopBar: the last poll reached every endpoint.
  const isLive = !Object.values(failed).some(Boolean)

  const hasLoaded = current !== null || history !== null || trend !== null || insights !== null
  const isLoading = !hasLoaded && !data.error

  if (isLoading) {
    return (
      <div className="min-h-screen bg-bg-base grid-bg">
        <TopBar lastUpdated={lastUpdated} isRefreshing={true} onRefresh={() => {}} />
        <main className="max-w-[1400px] mx-auto px-4 py-6 space-y-6">
          <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
            {[1, 2, 3, 4].map(i => (
              <div key={i} className="card p-5 animate-pulse">
                <div className="h-3 bg-bg-border rounded w-20 mb-3" />
                <div className="h-8 bg-bg-border rounded w-24" />
              </div>
            ))}
          </div>
          <div className="card p-5 animate-pulse">
            <div className="h-4 bg-bg-border rounded w-48 mb-4" />
            <div className="grid grid-cols-6 gap-3">
              {[1, 2, 3, 4, 5, 6].map(i => (
                <div key={i} className="h-16 bg-bg-border rounded" />
              ))}
            </div>
          </div>
          <div className="card p-5 animate-pulse h-64" />
        </main>
      </div>
    )
  }

  if (data.error) {
    return (
      <div className="min-h-screen bg-bg-base grid-bg">
        <TopBar
          lastUpdated={lastUpdated}
          isRefreshing={false}
          isLive={false}
          onRefresh={() => refresh()}
        />
        <main className="max-w-[1400px] mx-auto px-4 py-6">
          <div className="card p-8 text-center">
            <div className="text-accent-red text-lg font-display font-bold mb-2">
              Connection Error
            </div>
            <p className="text-text-muted text-sm font-mono mb-4">{data.error}</p>
            <button
              onClick={() => refresh()}
              className="px-4 py-2 rounded bg-accent-green/10 border border-accent-green/30 text-accent-green text-sm font-mono hover:bg-accent-green/20 transition-colors"
            >
              Retry
            </button>
          </div>
        </main>
      </div>
    )
  }

  return (
    <div className="min-h-screen bg-bg-base grid-bg">
      <TopBar
        lastUpdated={lastUpdated}
        isRefreshing={isRefreshing}
        isLive={isLive}
        onRefresh={() => refresh()}
      />

      <main className="max-w-[1400px] mx-auto px-4 py-6 space-y-6">

        {/* Row 1 — Stat Cards */}
        <StatCards
          current={current}
          trend={trend}
        />

        {/* Row 2 — Percentile strip */}
        {current && (
          <PercentileRow percentiles={current.percentiles} />
        )}

        {/* Row 3 — Recommendation Panel */}
        <RecommendationPanel />

        {/* Row 4 — Chart */}
        <FeeChart
          history={history}
          window={window}
          onWindowChange={handleWindowChange}
          unavailable={failed.history ?? false}
          onRetry={() => refresh()}
        />

        {/* Row 5 — Trend + Averages */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <TrendPanel
            trend={trend}
            unavailable={failed.trend ?? false}
            onRetry={() => refresh()}
          />
          <RollingAverages
            insights={insights}
            unavailable={failed.insights ?? false}
            onRetry={() => refresh()}
          />
        </div>

        {/* Footer */}
        <div className="text-center text-text-muted text-xs pb-8 font-mono">
          <span className="text-accent-green/50">◆</span>
          {' '}STELLAR TESTNET · POLLING EVERY 10s · BUILT WITH RUST + NEXT.JS
        </div>
      </main>
    </div>
  )
}
