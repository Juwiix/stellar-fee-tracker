'use client'

import { useState, useEffect } from 'react'
import { useRecommendation } from '@/lib/hooks/useRecommendation'
import { formatStroops } from '@/lib/utils'
import type { Urgency } from '@/lib/types'
import { FeeAlternatives } from './FeeAlternatives'

const URGENCY_OPTIONS: { value: Urgency; label: string }[] = [
  { value: 'low',    label: 'Low (cheapest)' },
  { value: 'medium', label: 'Medium (balanced)' },
  { value: 'high',   label: 'High (fast)' },
  { value: 'urgent', label: 'Urgent (next ledger)' },
]

const CONFIDENCE_OPTIONS = [70, 90, 95, 99] as const
type ConfidenceLevel = typeof CONFIDENCE_OPTIONS[number]

const LS_KEY = 'stellar_fee_confidence'

function loadConfidence(): ConfidenceLevel {
  if (typeof window === 'undefined') return 90
  const stored = localStorage.getItem(LS_KEY)
  const parsed = Number(stored)
  return (CONFIDENCE_OPTIONS as readonly number[]).includes(parsed)
    ? (parsed as ConfidenceLevel)
    : 90
}

const MIN_LEDGERS = 1
const MAX_LEDGERS = 100

export function RecommendationPanel() {
  const { recommend, result, loading, error, clear } = useRecommendation()
  const [urgency, setUrgency] = useState<Urgency>('medium')
  // Held as a string so the field can be cleared while typing. The request is
  // gated on `maxWaitValid`, so an empty field (Number('') === 0) or an
  // out-of-range value is never sent to the API.
  const [maxWait, setMaxWait] = useState('1')
  const [confidence, setConfidence] = useState<ConfidenceLevel>(90)

  const maxWaitLedgers = Number(maxWait)
  const maxWaitValid =
    Number.isInteger(maxWaitLedgers) &&
    maxWaitLedgers >= MIN_LEDGERS &&
    maxWaitLedgers <= MAX_LEDGERS

  // Hydrate from localStorage after mount
  useEffect(() => {
    setConfidence(loadConfidence())
  }, [])

  const handleConfidenceChange = (val: ConfidenceLevel) => {
    setConfidence(val)
    localStorage.setItem(LS_KEY, String(val))
  }

  const handleRecommend = async () => {
    if (!maxWaitValid) return
    await recommend(maxWaitLedgers, urgency, undefined, confidence)
  }

  return (
    <div className="rounded-xl border border-border-base bg-bg-card p-5 transition-all">
      <h3 className="text-sm font-semibold tracking-wide text-text-muted uppercase mb-4">
        Fee Recommendation
      </h3>

      <div className="space-y-3 mb-4">
        {/* Confidence selector — a radio-style group, so it is named by a
            group label rather than a <label> that cannot point at a control. */}
        <div role="group" aria-labelledby="confidence-label">
          <span id="confidence-label" className="text-xs text-text-muted block mb-1">
            Confidence
          </span>
          <div className="flex gap-1.5">
            {CONFIDENCE_OPTIONS.map((c) => (
              <button
                key={c}
                onClick={() => handleConfidenceChange(c)}
                aria-pressed={confidence === c}
                className={`flex-1 rounded-lg border py-1.5 text-xs font-medium transition-colors ${
                  confidence === c
                    ? 'border-accent-blue bg-accent-blue/10 text-accent-blue'
                    : 'border-border-base bg-bg-base text-text-muted hover:bg-bg-card'
                }`}
              >
                {c}%
              </button>
            ))}
          </div>
        </div>

        <div>
          <label htmlFor="urgency" className="text-xs text-text-muted block mb-1">
            Urgency
          </label>
          <select
            id="urgency"
            value={urgency}
            onChange={(e) => setUrgency(e.target.value as Urgency)}
            className="w-full rounded-lg border border-border-base bg-bg-base px-3 py-2 text-sm text-text-base focus:outline-none focus:ring-2 focus:ring-accent-blue/40"
          >
            {URGENCY_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>{opt.label}</option>
            ))}
          </select>
        </div>

        <div>
          <label htmlFor="max-wait" className="text-xs text-text-muted block mb-1">
            Max wait (ledgers)
          </label>
          <input
            id="max-wait"
            type="number"
            min={MIN_LEDGERS}
            max={MAX_LEDGERS}
            value={maxWait}
            onChange={(e) => setMaxWait(e.target.value)}
            aria-invalid={!maxWaitValid}
            aria-describedby={!maxWaitValid ? 'max-wait-hint' : undefined}
            className="w-full rounded-lg border border-border-base bg-bg-base px-3 py-2 text-sm text-text-base focus:outline-none focus:ring-2 focus:ring-accent-blue/40"
          />
          {!maxWaitValid && (
            <p id="max-wait-hint" className="text-xs text-red-400 mt-1">
              Enter a whole number between {MIN_LEDGERS} and {MAX_LEDGERS}.
            </p>
          )}
        </div>

        <button
          onClick={handleRecommend}
          disabled={loading || !maxWaitValid}
          className="w-full rounded-lg bg-accent-blue px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-accent-blue/90 disabled:opacity-50"
        >
          {loading ? 'Calculating...' : 'Get Recommendation'}
        </button>
      </div>

      {error && (
        <div className="rounded-lg bg-red-500/10 border border-red-500/20 px-3 py-2 mb-3">
          <p className="text-xs text-red-400">{error}</p>
        </div>
      )}

      {result && (
        <div className="space-y-3">
          <div className="rounded-lg bg-accent-green/10 border border-accent-green/20 p-3">
            <p className="text-2xl font-bold text-accent-green tabular-nums">
              {formatStroops(result.fee_in_stroops)}
            </p>
            <p className="text-xs text-text-muted mt-1">
              Recommended fee · ~{result.estimated_wait_ledgers} ledger{result.estimated_wait_ledgers !== 1 ? 's' : ''} wait
              {' · '}{(result.confidence * 100).toFixed(0)}% confidence
            </p>
            {result.network_condition !== 'unknown' && (
              <p className="text-xs text-text-muted mt-0.5">
                Network: <span className="capitalize">{result.network_condition}</span>
              </p>
            )}
          </div>

          <FeeAlternatives alternatives={result.alternatives} />

          <button
            onClick={clear}
            className="w-full rounded-lg border border-border-base px-4 py-1.5 text-xs text-text-muted transition-colors hover:bg-bg-base"
          >
            Clear
          </button>
        </div>
      )}
    </div>
  )
}
