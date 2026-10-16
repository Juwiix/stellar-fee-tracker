// ---- /fees/current ----
export interface PercentileFees {
  p10: string
  p20: string
  p30: string
  p40: string
  p50: string
  p60: string
  p70: string
  p80: string
  p90: string
  p95: string
  p99: string
}

export interface CurrentFeeResponse {
  base_fee: string
  min_fee: string
  max_fee: string
  avg_fee: string
  percentiles: PercentileFees
}

// ---- /fees/history ----
export interface FeeDataPoint {
  fee_amount: number
  timestamp: string
  transaction_hash: string
  ledger_sequence: number
}

export interface FeeSummary {
  min: number
  max: number
  avg: number
  p50: number
  p95: number
}

export interface FeeHistoryResponse {
  window: string
  from: string
  to: string
  data_points: number
  fees: FeeDataPoint[]
  summary: FeeSummary
}

// ---- /fees/trend ----
export interface TrendChanges {
  '1h_pct':  number | null
  '6h_pct':  number | null
  '24h_pct': number | null
}

export interface FeeTrendResponse {
  status: 'Normal' | 'Rising' | 'Congested' | 'Declining'
  trend_strength: 'Weak' | 'Moderate' | 'Strong'
  changes: TrendChanges
  recent_spike_count: number
  predicted_congestion_minutes: number | null
  last_updated: string
}

// ---- /insights ----
export interface AverageResult {
  value: number
  sample_count: number
  is_partial: boolean
  calculated_at: string
}

export interface RollingAverages {
  short_term:  AverageResult
  medium_term: AverageResult
  long_term:   AverageResult
}

export interface ExtremeValue {
  value: number
  timestamp: string
  transaction_hash: string
}

export interface FeeExtremes {
  current_min:  ExtremeValue
  current_max:  ExtremeValue
  period_start: string
  period_end:   string
}

export interface InsightsResponse {
  rolling_averages: RollingAverages
  extremes:         FeeExtremes
  last_updated:     string
  // these exist in the response but we can ignore them for now
  congestion_trends?: unknown
  data_quality?:      unknown
}
// ---- /fees/recommend ----
export interface FeeAlternative {
  fee: string
  estimated_wait_ledgers: number
  confidence: number
  label: string
}

export interface RecommendResponse {
  recommended_fee: string
  fee_in_stroops: number
  estimated_wait_ledgers: number
  confidence: number
  network_condition: string
  alternatives: FeeAlternative[]
}

export type Urgency = "low" | "medium" | "high" | "urgent"

export interface RecommendRequest {
  target_ledgers?: number
  urgency?: Urgency
  max_fee?: string
  /** Desired confidence level as a fraction (e.g. 0.90 for 90%) */
  confidence?: number
}

/** Alias exported for consumers that prefer the Response name */
export type RecommendationResponse = RecommendResponse

// ---- Issue #75 aliases ----
/** Alias for CurrentFeeResponse as specified in the shared lib layer issue */
export type FeeStats = CurrentFeeResponse
/** Alias for FeeHistoryResponse as specified in the shared lib layer issue */
export type FeeHistory = FeeHistoryResponse
