import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import type { CurrentFeeResponse, FeeTrendResponse } from "@/lib/types";
import { StatCards } from "./StatCards";

const CURRENT: CurrentFeeResponse = {
  base_fee: "100",
  min_fee: "100",
  max_fee: "900",
  avg_fee: "2500",
  percentiles: {
    p10: "100",
    p20: "100",
    p30: "120",
    p40: "150",
    p50: "200",
    p60: "300",
    p70: "400",
    p80: "600",
    p90: "800",
    p95: "900",
    p99: "1000",
  },
};

const TREND: FeeTrendResponse = {
  status: "Rising",
  trend_strength: "Moderate",
  changes: { "1h_pct": 12.5, "6h_pct": null, "24h_pct": -3 },
  recent_spike_count: 3,
  predicted_congestion_minutes: 8,
  last_updated: "2026-01-01T00:00:00Z",
};

describe("StatCards", () => {
  it("formats the current base and average fee as stroops", () => {
    render(<StatCards current={CURRENT} trend={TREND} />);

    expect(screen.getByText("Base Fee")).toBeInTheDocument();
    expect(screen.getByText("100 str")).toBeInTheDocument();
    expect(screen.getByText("Avg Fee")).toBeInTheDocument();
    expect(screen.getByText("2,500 str")).toBeInTheDocument();
  });

  it("shows the trend status and strength", () => {
    render(<StatCards current={CURRENT} trend={TREND} />);

    expect(screen.getByText("Rising")).toBeInTheDocument();
    expect(screen.getByText("Moderate trend")).toBeInTheDocument();
  });

  it("flags spikes and says all clear otherwise", () => {
    const { unmount } = render(<StatCards current={CURRENT} trend={TREND} />);
    expect(screen.getByText("3")).toBeInTheDocument();
    expect(screen.getByText("fee anomalies detected")).toBeInTheDocument();
    unmount();

    render(
      <StatCards current={CURRENT} trend={{ ...TREND, recent_spike_count: 0 }} />,
    );
    expect(screen.getByText("all clear")).toBeInTheDocument();
  });

  it("falls back to an em dash while the current fee response is missing", () => {
    render(<StatCards current={null} trend={TREND} />);

    expect(screen.getAllByText("—")).toHaveLength(2);
  });
});
