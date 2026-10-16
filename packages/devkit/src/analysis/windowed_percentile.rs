//! Computes percentiles (p10/p50/p90/p99) over a fixed-size rolling window
//! of fee values, combining windowing and percentile computation into one
//! type.
//!
//! RollingWindow (#44) and PercentileTable (#43) don't exist in this crate
//! yet, so windowing is implemented inline here rather than composed from
//! them; the public surface (`push`, `percentiles`) is what a later
//! composed version should preserve.
//!
//! Closes #770.

use std::collections::VecDeque;

#[allow(dead_code)]
#[path = "../utilities/percentile_table.rs"]
mod percentile_table;

pub use percentile_table::percentile;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Percentiles {
    pub p10: f64,
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
}

pub struct WindowedPercentile {
    capacity: usize,
    values: VecDeque<f64>,
}

impl WindowedPercentile {
    pub fn new(capacity: usize) -> Self {
        Self { capacity, values: VecDeque::with_capacity(capacity) }
    }

    pub fn push(&mut self, value: f64) {
        if self.capacity == 0 {
            return;
        }
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    /// Percentiles over the values currently in the window.
    pub fn percentiles(&self) -> Percentiles {
        let mut sorted: Vec<f64> = self.values.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        Percentiles {
            p10: percentile(&sorted, 10.0),
            p50: percentile(&sorted, 50.0),
            p90: percentile(&sorted, 90.0),
            p99: percentile(&sorted, 99.0),
        }
    }
}
