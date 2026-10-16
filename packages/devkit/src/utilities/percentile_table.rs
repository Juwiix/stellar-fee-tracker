//! Computes p10/p50/p90/p99 from a slice of fee values.
//!
//! Closes #768.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PercentileTable {
    pub p10: f64,
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
}

pub fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (pct / 100.0) * (sorted.len() as f64 - 1.0);
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    if lower == upper {
        sorted[lower]
    } else {
        let frac = rank - lower as f64;
        sorted[lower] + (sorted[upper] - sorted[lower]) * frac
    }
}

impl PercentileTable {
    pub fn compute(values: &[f64]) -> Self {
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        Self {
            p10: percentile(&sorted, 10.0),
            p50: percentile(&sorted, 50.0),
            p90: percentile(&sorted, 90.0),
            p99: percentile(&sorted, 99.0),
        }
    }
}
