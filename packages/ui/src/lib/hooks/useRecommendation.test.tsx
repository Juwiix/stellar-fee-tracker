import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { api } from "@/lib/api";
import type { RecommendResponse } from "@/lib/types";
import { useRecommendation } from "./useRecommendation";

vi.mock("@/lib/api", () => ({
  api: { recommend: vi.fn() },
}));

const recommendMock = vi.mocked(api.recommend);

const RESPONSE: RecommendResponse = {
  recommended_fee: "250",
  fee_in_stroops: 250,
  estimated_wait_ledgers: 2,
  confidence: 0.9,
  network_condition: "normal",
  alternatives: [
    { fee: "100", estimated_wait_ledgers: 6, confidence: 0.7, label: "economy" },
  ],
};

describe("useRecommendation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("exposes the recommendation and converts confidence to a fraction", async () => {
    recommendMock.mockResolvedValue(RESPONSE);

    const { result } = renderHook(() => useRecommendation());
    expect(result.current.loading).toBe(false);

    await act(async () => {
      await result.current.recommend(5, "high", undefined, 95);
    });

    expect(recommendMock).toHaveBeenCalledWith({
      target_ledgers: 5,
      urgency: "high",
      max_fee: undefined,
      confidence: 0.95,
    });
    expect(result.current.result).toEqual(RESPONSE);
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it("reports the API error message instead of throwing", async () => {
    recommendMock.mockRejectedValue(new Error("API error 503 on /fees/recommend"));

    const { result } = renderHook(() => useRecommendation());
    await act(async () => {
      await result.current.recommend();
    });

    expect(result.current.error).toBe("API error 503 on /fees/recommend");
    expect(result.current.result).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it("falls back to a generic message for non-Error rejections", async () => {
    recommendMock.mockRejectedValue("boom");

    const { result } = renderHook(() => useRecommendation());
    await act(async () => {
      await result.current.recommend();
    });

    expect(result.current.error).toBe("Failed to get fee recommendation");
  });

  it("clears the previous result and error", async () => {
    recommendMock.mockResolvedValue(RESPONSE);

    const { result } = renderHook(() => useRecommendation());
    await act(async () => {
      await result.current.recommend();
    });
    await waitFor(() => expect(result.current.result).toEqual(RESPONSE));

    act(() => {
      result.current.clear();
    });

    expect(result.current.result).toBeNull();
    expect(result.current.error).toBeNull();
  });
});
