'use client'

import type { InsightsResponse } from '@/lib/types'
import { formatStroops, cn } from '@/lib/utils'

interface Props {
  insights: InsightsResponse | null
  /** The most recent /insights request failed — never render 'Loading' for it. */
  unavailable?: boolean
  onRetry?: () => void
}

export function RollingAverages({ insights, unavailable = false, onRetry }: Props) {
  if (!insights) {
    return (
      <div className="card p-5 flex flex-col items-center justify-center gap-3 text-text-muted text-sm font-mono h-48">
        <span>
          {unavailable ? 'Rolling averages unavailable' : 'Loading insights...'}
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

  const { rolling_averages, extremes } = insights

  const rows = [
    {
      label:   'Short-term avg',
      sub:     '~5 min window',
      result:  rolling_averages.short_term,
      accent:  'text-accent-green',
      border:  'border-accent-green/20',
    },
    {
      label:   'Medium-term avg',
      sub:     '~1 hour window',
      result:  rolling_averages.medium_term,
      accent:  'text-accent-cyan',
      border:  'border-accent-cyan/20',
    },
    {
      label:   'Long-term avg',
      sub:     '~24 hour window',
      result:  rolling_averages.long_term,
      accent:  'text-accent-yellow',
      border:  'border-accent-yellow/20',
    },
  ]

  return (
    <div className="card p-5 space-y-5">
      <div className="text-xs font-mono text-text-muted tracking-widest uppercase">
        Rolling Averages
      </div>

      <div className="space-y-3">
        {rows.map(({ label, sub, result, accent, border }) => (
          <div
            key={label}
            className={cn('flex items-center justify-between py-3 px-3 rounded border bg-bg-base', border)}
          >
            <div>
              <div className="text-xs text-text-secondary font-mono">{label}</div>
              <div className="text-xs text-text-muted font-mono">
                {sub}
                {result.is_partial && <span className="text-accent-yellow ml-1">(partial)</span>}
              </div>
            </div>
            <div className="text-right">
              <div className={cn('text-lg font-display font-bold ticker', accent)}>
                {formatStroops(result.value)}
              </div>
              <div className="text-xs text-text-muted font-mono">
                {result.sample_count} samples
              </div>
            </div>
          </div>
        ))}
      </div>

      {/* Extremes */}
      <div className="pt-2 border-t border-bg-border space-y-2">
        <div className="text-xs font-mono text-text-muted tracking-widest uppercase">
          Period Extremes
        </div>
        <div className="flex justify-between text-xs font-mono">
          <div>
            <span className="text-text-muted">Min  </span>
            <span className="text-accent-green font-bold ticker">
              {formatStroops(extremes.current_min.value)}
            </span>
          </div>
          <div>
            <span className="text-text-muted">Max  </span>
            <span className="text-accent-red font-bold ticker">
              {formatStroops(extremes.current_max.value)}
            </span>
          </div>
        </div>
      </div>
    </div>
  )
}