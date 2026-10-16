'use client'

import { TrendingUp, TrendingDown, Minus } from 'lucide-react'
import type { FeeTrendResponse } from '@/lib/types'
import { pctColor, pctArrow, congestionColor, cn } from '@/lib/utils'

interface Props {
  trend: FeeTrendResponse | null
  /** The most recent /fees/trend request failed — never render 'Loading' for it. */
  unavailable?: boolean
  onRetry?: () => void
}

export function TrendPanel({ trend, unavailable = false, onRetry }: Props) {
  if (!trend) {
    return (
      <div className="card p-5 flex flex-col items-center justify-center gap-3 text-text-muted text-sm font-mono h-48">
        <span>
          {unavailable ? 'Trend data unavailable' : 'Loading trend data...'}
        </span>
        {unavailable && onRetry && (
          <button
            onClick={onRetry}
            className="px-3 py-1.5 rounded bg-accent-green/10 border border-accent-green/30 text-accent-green text-xs font-mono hover:bg-accent-green/20 transition-colors"
          >
            Retry
          </button>
        )}
      </div>
    )
  }

  const { status, trend_strength, changes, recent_spike_count, predicted_congestion_minutes } = trend

  const StatusIcon =
    status === 'Rising' || status === 'Congested' ? TrendingUp :
    status === 'Declining' ? TrendingDown : Minus

  const strengthDots = trend_strength === 'Strong' ? 3 : trend_strength === 'Moderate' ? 2 : 1

  return (
    <div className="card p-5 space-y-5">
      <div className="text-xs font-mono text-text-muted tracking-widest uppercase">
        Trend Analysis
      </div>

      {/* Status + strength */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <StatusIcon size={20} className={congestionColor(status)} />
          <span className={cn('text-2xl font-display font-bold', congestionColor(status))}>
            {status}
          </span>
        </div>
        <div className="flex items-center gap-1.5">
          {[1, 2, 3].map(i => (
            <div
              key={i}
              className={cn(
                'w-2 h-2 rounded-full',
                i <= strengthDots ? 'bg-accent-cyan' : 'bg-bg-border'
              )}
            />
          ))}
          <span className="text-xs text-text-secondary font-mono ml-1">{trend_strength}</span>
        </div>
      </div>

      {/* Pct changes */}
      <div className="space-y-2">
        {([
          ['1H',  changes['1h_pct']],
          ['6H',  changes['6h_pct']],
          ['24H', changes['24h_pct']],
        ] as [string, number | null][]).map(([label, pct]) => (
          <div key={label} className="flex items-center justify-between py-2 border-b border-bg-border last:border-0">
            <span className="text-xs text-text-muted font-mono">{label} change</span>
            <span className={cn('text-sm font-mono font-bold', pctColor(pct))}>
              {pctArrow(pct)}
            </span>
          </div>
        ))}
      </div>

      {/* Extras */}
      <div className="flex items-center justify-between text-xs font-mono">
        <span className="text-text-muted">Recent spikes</span>
        <span className={recent_spike_count > 0 ? 'text-accent-yellow' : 'text-accent-green'}>
          {recent_spike_count}
        </span>
      </div>

      {predicted_congestion_minutes !== null && (
        <div className="flex items-center justify-between text-xs font-mono">
          <span className="text-text-muted">Predicted congestion</span>
          <span className="text-accent-red">{predicted_congestion_minutes}m</span>
        </div>
      )}
    </div>
  )
}