//! Fixed-size sliding window of recent fee values.
//!
//! Closes #769.

use std::collections::VecDeque;

pub struct RollingWindow {
    capacity: usize,
    values: VecDeque<f64>,
}

impl RollingWindow {
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

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn snapshot(&self) -> Vec<f64> {
        self.values.iter().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_zero_never_grows() {
        let mut window = RollingWindow::new(0);
        assert_eq!(window.len(), 0);
        assert!(window.is_empty());
        window.push(100.0);
        assert_eq!(window.len(), 0);
        assert!(window.is_empty());
        assert!(window.snapshot().is_empty());
    }

    #[test]
    fn normal_capacity_slides() {
        let mut window = RollingWindow::new(2);
        window.push(1.0);
        window.push(2.0);
        window.push(3.0);
        assert_eq!(window.snapshot(), vec![2.0, 3.0]);
    }
}
