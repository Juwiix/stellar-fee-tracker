import { describe, expect, it } from "vitest";
import { formatStroops, pctArrow, pctColor } from "./utils";

describe("formatStroops", () => {
  it("keeps small values in stroops", () => {
    expect(formatStroops(100)).toBe("100 str");
    expect(formatStroops(999)).toBe("999 str");
  });

  it("groups thousands", () => {
    expect(formatStroops(1_000)).toBe("1,000 str");
    expect(formatStroops(9_999_999)).toBe("9,999,999 str");
  });

  it("switches to XLM from 10,000,000 stroops up", () => {
    expect(formatStroops(10_000_000)).toBe("1.00 XLM");
    expect(formatStroops(12_345_678)).toBe("1.23 XLM");
  });

  it("accepts the string form the API returns", () => {
    expect(formatStroops("2500")).toBe("2,500 str");
    expect(formatStroops("10000000")).toBe("1.00 XLM");
  });
});

describe("pctArrow", () => {
  it("marks increases and decreases", () => {
    expect(pctArrow(12.34)).toBe("▲ +12.3%");
    expect(pctArrow(-5)).toBe("▼ -5.0%");
  });

  it("marks a flat change separately from a missing one", () => {
    expect(pctArrow(0)).toBe("→ 0.0%");
    expect(pctArrow(null)).toBe("—");
  });
});

describe("pctColor", () => {
  it("escalates once a change passes ten percent", () => {
    expect(pctColor(11)).toBe("text-accent-red");
    expect(pctColor(5)).toBe("text-accent-yellow");
    expect(pctColor(-11)).toBe("text-accent-green");
    expect(pctColor(-5)).toBe("text-accent-cyan");
  });

  it("stays neutral when the change is unknown or zero", () => {
    expect(pctColor(null)).toBe("text-text-secondary");
    expect(pctColor(0)).toBe("text-text-secondary");
  });
});
