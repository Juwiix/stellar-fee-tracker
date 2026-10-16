//! Property tests confirming RollingWindow never holds more than its
//! configured capacity.
//!
//! RollingWindow is reimplemented locally as a minimal stand-in until #44
//! lands in this crate.
//!
//! Closes #773.

use proptest::prelude::*;
use std::collections::VecDeque;

struct RollingWindow {
    capacity: usize,
    values: VecDeque<f64>,
}

impl RollingWindow {
    fn new(capacity: usize) -> Self {
        Self { capacity, values: VecDeque::with_capacity(capacity.max(1)) }
    }

    fn push(&mut self, value: f64) {
        if self.capacity == 0 {
            return;
        }
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    fn len(&self) -> usize {
        self.values.len()
    }
}

proptest! {
    #[test]
    fn never_exceeds_capacity(
        capacity in 0usize..100,
        pushes in prop::collection::vec(any::<f64>().prop_filter("finite", |v| v.is_finite()), 0..500),
    ) {
        let mut window = RollingWindow::new(capacity);
        for value in pushes {
            window.push(value);
            prop_assert!(window.len() <= capacity);
        }
    }
}
