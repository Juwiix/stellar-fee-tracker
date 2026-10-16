'use client'

import React, { RefObject, useEffect, useRef, useState } from "react";

// ─── Animated counter ─────────────────────────────────────────────────────────
function useCounter(target: number, duration = 1200) {
  const [val, setVal] = useState(0);
  const [started, setStarted] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const obs = new IntersectionObserver(
      ([e]) => {
        if (e.isIntersecting) setStarted(true);
      },
      { threshold: 0.3 },
    );
    if (ref.current) obs.observe(ref.current);
    return () => obs.disconnect();
  }, []);
  useEffect(() => {
    if (!started) return;
    // The count-up is decoration: with reduced motion requested, show the final
    // value immediately instead of animating to it.
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setVal(target);
      return;
    }
    let start: number;
    const step = (ts: number) => {
      if (!start) start = ts;
      const p = Math.min((ts - start) / duration, 1);
      setVal(Math.floor(p * target));
      if (p < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  }, [started, target, duration]);
  return { val, ref };
}

function StatStrip() {
  const { val: txCount, ref: txRef } = useCounter(10000);
  const { val: ledgerCount, ref: ledgerRef } = useCounter(1351393);
  const { val: pollRate, ref: pollRef } = useCounter(10);
  return (
    <div className="relative z-10 border-y border-[#1e2a36] bg-[#0d1117] py-4 overflow-hidden">
      <div className="flex gap-12 px-6 max-w-6xl mx-auto flex-wrap md:flex-nowrap">
        {[
          {
            label: "TRANSACTIONS TRACKED",
            ref: txRef,
            val: txCount,
            suffix: "+",
            color: "#00ff9d",
          },
          {
            label: "LEDGERS PROCESSED",
            ref: ledgerRef,
            val: ledgerCount,
            suffix: "",
            color: "#00d4ff",
          },
          {
            label: "POLL INTERVAL",
            ref: pollRef,
            val: pollRate,
            suffix: "s",
            color: "#ffd23f",
          },
        ].map(({ label, ref: r, val, suffix, color }) => (
          <div
            key={label}
            ref={r as RefObject<HTMLDivElement>}
            className="flex-1 min-w-[160px]"
          >
            <div className="text-[9px] text-[#3d5a73] tracking-widest mb-1">
              {label}
            </div>
            <div
              className="text-2xl font-bold"
              style={{ fontFamily: "'Space Mono', monospace", color }}
            >
              {val.toLocaleString()}
              {suffix}
            </div>
          </div>
        ))}
        <div className="flex-1 min-w-[160px]">
          <div className="text-[9px] text-[#3d5a73] tracking-widest mb-1">
            OPEN SOURCE
          </div>
          <div
            className="text-2xl font-bold text-[#e2eaf4]"
            style={{ fontFamily: "'Space Mono', monospace" }}
          >
            MIT
          </div>
        </div>
      </div>
    </div>
  );
}

export default StatStrip;
