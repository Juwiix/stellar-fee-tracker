'use client'

import type { PercentileFees } from '@/lib/types'
import { formatStroops } from '@/lib/utils'

interface Props {
  percentiles: PercentileFees
}

const LABELS: { key: keyof PercentileFees; label: string; color: string }[] = [
  { key: 'p10', label: 'P10', color: 'text-accent-green  border-accent-green/40' },
  { key: 'p20', label: 'P20', color: 'text-accent-green  border-accent-green/30' },
  { key: 'p30', label: 'P30', color: 'text-accent-green  border-accent-green/20' },
  { key: 'p40', label: 'P40', color: 'text-accent-cyan   border-accent-cyan/30'  },
  { key: 'p50', label: 'P50', color: 'text-accent-cyan   border-accent-cyan/40'  },
  { key: 'p60', label: 'P60', color: 'text-accent-cyan   border-accent-cyan/30'  },
  { key: 'p70', label: 'P70', color: 'text-accent-yellow border-accent-yellow/30' },
  { key: 'p80', label: 'P80', color: 'text-accent-yellow border-accent-yellow/40' },
  { key: 'p90', label: 'P90', color: 'text-accent-red    border-accent-red/30'   },
  { key: 'p95', label: 'P95', color: 'text-accent-red    border-accent-red/50'   },
  { key: 'p99', label: 'P99', color: 'text-accent-red    border-accent-red/70'   },
]

export function PercentileRow({ percentiles }: Props) {
  return (
    <div className="card p-4">
      <div className="text-xs font-mono text-text-muted tracking-widest uppercase mb-4">
        Fee Percentiles · Last 5 Ledgers
      </div>
      <div className="grid grid-cols-3 md:grid-cols-6 gap-3">
        {LABELS.map(({ key, label, color }) => (
          <div
            key={key}
            className={`flex flex-col items-center gap-1 py-3 rounded border bg-bg-base ${color}`}
          >
            <span className="text-xs text-text-muted font-mono">{label}</span>
            <span className="text-sm font-display font-bold ticker">
              {formatStroops(percentiles[key])}
            </span>
          </div>
        ))}
      </div>
    </div>
  )
}