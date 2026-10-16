'use client'

import { TrendingUp, TrendingDown, Minus, AlertTriangle, Layers, BarChart2 } from 'lucide-react'
import type { CurrentFeeResponse, FeeTrendResponse } from '@/lib/types'
import { formatStroops, congestionBg, cn } from '@/lib/utils'

interface Props {
  current: CurrentFeeResponse | null
  trend:   FeeTrendResponse   | null
}

function StatCard({
  label,
  value,
  sub,
  icon: Icon,
  accent = false,
  className = '',
}: {
  label:     string
  value:     string
  sub?:      React.ReactNode
  icon:      React.ElementType
  accent?:   boolean
  className?: string
}) {
  return (
    <div className={cn(
      'card p-5 flex flex-col gap-3 animate-slide-up',
      className,
    )}>
      <div className="flex items-center justify-between">
        <span className="text-xs font-mono text-text-muted tracking-widest uppercase">{label}</span>
        <Icon size={14} className={accent ? 'text-accent-green' : 'text-text-muted'} />
      </div>
      <div className={cn(
        'text-3xl font-display font-bold ticker',
        accent ? 'text-accent-green glow-green' : 'text-text-primary'
      )}>
        {value}
      </div>
      {sub && (
        <div className="text-xs text-text-secondary font-mono">{sub}</div>
      )}
    </div>
  )
}

export function StatCards({ current, trend }: Props) {
  const baseFee    = current ? formatStroops(current.base_fee)  : '—'
  const avgFee     = current ? formatStroops(current.avg_fee)   : '—'
  const status     = trend?.status ?? 'Normal'
  const spikes     = trend?.recent_spike_count ?? 0
  const strength   = trend?.trend_strength ?? '—'

  const TrendIcon = status === 'Rising' || status === 'Congested'
    ? TrendingUp
    : status === 'Declining'
    ? TrendingDown
    : Minus

  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
      <StatCard
        label="Base Fee"
        value={baseFee}
        sub={<span className="text-text-muted">last ledger</span>}
        icon={Layers}
        accent
      />
      <StatCard
        label="Avg Fee"
        value={avgFee}
        sub={<span className="text-text-muted">mode over 5 ledgers</span>}
        icon={BarChart2}
      />
      <StatCard
        label="Network Status"
        value={status}
        sub={<span className="text-text-muted">{strength} trend</span>}
        icon={TrendIcon}
        className={congestionBg(status)}
      />
      <StatCard
        label="Recent Spikes"
        value={String(spikes)}
        sub={
          spikes > 0
            ? <span className="text-accent-yellow">fee anomalies detected</span>
            : <span className="text-accent-green">all clear</span>
        }
        icon={AlertTriangle}
      />
    </div>
  )
}