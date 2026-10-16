"use client";

import {
  ResponsiveContainer,
  LineChart,
  Line,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  ReferenceLine,
} from "recharts";
import type { FeeHistoryResponse, FeeDataPoint } from "@/lib/types";
import { cn } from "@/lib/utils";

interface Props {
  history: FeeHistoryResponse | null;
  window: "1h" | "6h" | "24h";
  onWindowChange: (w: "1h" | "6h" | "24h") => void;
  /** The most recent /fees/history request failed. */
  unavailable?: boolean;
  onRetry?: () => void;
}

const WINDOWS: ("1h" | "6h" | "24h")[] = ["1h", "6h", "24h"];

function formatTime(iso: string, win: string): string {
  const d = new Date(iso);
  if (win === "24h") {
    return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }
  return d.toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

interface CustomTooltipProps {
  active?: boolean;
  payload?: Array<{ value: number }>;
  label?: string;
}

function CustomTooltip({ active, payload, label }: CustomTooltipProps) {
  if (!active || !payload?.length) return null;
  const fee = payload[0]?.value as number;
  return (
    <div className="card px-3 py-2 text-xs font-mono border-accent-green/30">
      <div className="text-text-muted mb-1">{label}</div>
      <div className="text-accent-green font-bold">
        {fee?.toLocaleString()} stroops
      </div>
    </div>
  );
}

export function FeeChart({
  history,
  window,
  onWindowChange,
  unavailable = false,
  onRetry,
}: Props) {
  const points = history?.fees ?? [];

  // Downsample for readability if too many points. The last point is always
  // kept: it is the newest ledger and the likeliest place for a spike, so
  // dropping it would hide exactly what the chart is for.
  const maxPoints = 200;
  const step =
    points.length > maxPoints ? Math.ceil(points.length / maxPoints) : 1;
  const sampled =
    step === 1 ? points : points.filter((_, i) => i % step === 0);
  const lastPoint = points[points.length - 1];
  const visiblePoints =
    lastPoint && sampled[sampled.length - 1] !== lastPoint
      ? [...sampled, lastPoint]
      : sampled;
  const chartData = visiblePoints.map((p: FeeDataPoint) => ({
    time: formatTime(p.timestamp, window),
    // Formatted times repeat across a 24h window, so the tooltip label uses the
    // full ISO timestamp the point actually came from.
    iso: p.timestamp,
    fee: p.fee_amount,
    ledger: p.ledger_sequence,
  }));

  const avg = history?.summary.avg ?? 0;
  const p95 = history?.summary.p95 ?? 0;
  const count = history?.data_points ?? 0;

  return (
    <div className="card p-5">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <div className="text-xs font-mono text-text-muted tracking-widest uppercase">
            Fee History
          </div>
          {count > 0 && (
            <div className="text-xs text-text-secondary font-mono mt-1">
              {count.toLocaleString()} transactions
              <span className="text-text-muted"> · avg </span>
              <span className="text-accent-cyan">{avg.toFixed(0)} str</span>
              <span className="text-text-muted"> · p95 </span>
              <span className="text-accent-yellow">
                {p95.toLocaleString()} str
              </span>
            </div>
          )}
        </div>

        {/* Window toggle */}
        <div className="flex gap-1">
          {WINDOWS.map((w) => (
            <button
              key={w}
              onClick={() => onWindowChange(w)}
              className={cn(
                "px-3 py-1 text-xs font-mono rounded border transition-colors",
                window === w
                  ? "bg-accent-green/10 border-accent-green/50 text-accent-green"
                  : "border-bg-border text-text-muted hover:text-text-secondary hover:border-bg-border/80",
              )}
            >
              {w.toUpperCase()}
            </button>
          ))}
        </div>
      </div>

      {/* Chart */}
      {chartData.length === 0 ? (
        <div className="h-64 flex flex-col items-center justify-center gap-3 text-text-muted text-sm font-mono">
          <span>
            {unavailable && !history
              ? "Fee history unavailable"
              : "No data yet — waiting for first poll cycle..."}
          </span>
          {unavailable && !history && onRetry && (
            <button
              onClick={onRetry}
              className="px-3 py-1.5 rounded bg-accent-green/10 border border-accent-green/30 text-accent-green text-xs font-mono hover:bg-accent-green/20 transition-colors"
            >
              Retry
            </button>
          )}
        </div>
      ) : (
        <ResponsiveContainer width="100%" height={280}>
          <LineChart
            data={chartData}
            margin={{ top: 4, right: 4, bottom: 4, left: 0 }}
          >
            <CartesianGrid strokeDasharray="3 3" stroke="#1e2a36" />
            <XAxis
              dataKey="time"
              tick={{
                fill: "#3d5a73",
                fontSize: 10,
                fontFamily: "JetBrains Mono",
              }}
              tickLine={false}
              axisLine={{ stroke: "#1e2a36" }}
              interval="preserveStartEnd"
            />
            <YAxis
              tick={{
                fill: "#3d5a73",
                fontSize: 10,
                fontFamily: "JetBrains Mono",
              }}
              tickLine={false}
              axisLine={false}
              tickFormatter={(v) => `${v.toLocaleString()}`}
              width={70}
            />
            <Tooltip
              content={<CustomTooltip />}
              labelFormatter={(_label, payload) =>
                (payload?.[0]?.payload as { iso?: string } | undefined)?.iso ?? ""
              }
            />
            {avg > 0 && (
              <ReferenceLine
                y={avg}
                stroke="#00d4ff"
                strokeDasharray="4 4"
                strokeOpacity={0.5}
                label={{
                  value: "avg",
                  fill: "#00d4ff",
                  fontSize: 9,
                  fontFamily: "JetBrains Mono",
                }}
              />
            )}
            {p95 > 0 && (
              <ReferenceLine
                y={p95}
                stroke="#ffd23f"
                strokeDasharray="4 4"
                strokeOpacity={0.4}
                label={{
                  value: "p95",
                  fill: "#ffd23f",
                  fontSize: 9,
                  fontFamily: "JetBrains Mono",
                }}
              />
            )}
            <Line
              type="monotone"
              dataKey="fee"
              stroke="#00ff9d"
              strokeWidth={1.5}
              dot={false}
              activeDot={{
                r: 4,
                fill: "#00ff9d",
                stroke: "#080b0f",
                strokeWidth: 2,
              }}
            />
          </LineChart>
        </ResponsiveContainer>
      )}
    </div>
  );
}
