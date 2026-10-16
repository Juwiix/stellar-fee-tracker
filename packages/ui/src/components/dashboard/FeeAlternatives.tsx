'use client'

import { useEffect, useState } from 'react'
import type { FeeAlternative } from '@/lib/types'
import { formatStroops } from '@/lib/utils'

const TIER_STYLES: Record<string, { border: string; label: string }> = {
  economy: { border: 'border-cyan-500/30',   label: 'text-cyan-400'   },
  standard:{ border: 'border-green-500/30',  label: 'text-green-400'  },
  fast:    { border: 'border-yellow-500/30', label: 'text-yellow-400' },
}

function tierStyle(label: string) {
  const key = label.toLowerCase()
  return TIER_STYLES[key] ?? { border: 'border-bg-border', label: 'text-text-primary' }
}

interface Props {
  alternatives: FeeAlternative[]
}

export function FeeAlternatives({ alternatives }: Props) {
  // Outcome of the last copy attempt, or null. Drives both the visible
  // confirmation next to the tier and the aria-live message below.
  const [feedback, setFeedback] = useState<{ label: string; ok: boolean } | null>(
    null,
  )

  // Clear the confirmation after a moment so it reads as transient feedback.
  useEffect(() => {
    if (!feedback) return
    const id = window.setTimeout(() => setFeedback(null), 2_000)
    return () => window.clearTimeout(id)
  }, [feedback])

  if (!alternatives.length) return null

  const handleCopy = (label: string, fee: string) => {
    navigator.clipboard.writeText(fee).then(
      () => setFeedback({ label, ok: true }),
      () => setFeedback({ label, ok: false }),
    )
  }

  return (
    <div>
      <p className="text-xs text-text-muted mb-2 font-medium">Alternatives</p>
      {/* Copy result announced to screen readers. */}
      <span role="status" aria-live="polite" className="sr-only">
        {feedback
          ? feedback.ok
            ? `Copied the ${feedback.label} fee`
            : `Could not copy the ${feedback.label} fee`
          : ''}
      </span>
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-2">
        {alternatives.map((alt) => {
          const { border, label: labelColor } = tierStyle(alt.label)
          return (
            <button
              key={alt.label}
              onClick={() => handleCopy(alt.label, alt.fee)}
              title="Click to copy fee"
              className={`rounded-lg border ${border} bg-bg-base px-2 py-2.5 text-left transition-colors hover:bg-bg-card`}
            >
              <p className={`text-xs font-semibold capitalize ${labelColor}`}>{alt.label}</p>
              <p className="text-sm font-bold text-text-primary tabular-nums mt-1">
                {formatStroops(Number(alt.fee))}
              </p>
              <p className="text-xs text-text-muted mt-0.5">
                {(alt.confidence * 100).toFixed(0)}% · ~{alt.estimated_wait_ledgers}L
              </p>
            </button>
          )
        })}
      </div>
    </div>
  )
}
