use crate::simulation::fee_model::FeePoint;
use std::collections::BTreeMap;

/// Result of comparing two fee data sets.
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// Label for the first data set.
    pub label_a: String,
    /// Label for the second data set.
    pub label_b: String,
    /// Number of points in set A.
    pub count_a: usize,
    /// Number of points in set B.
    pub count_b: usize,
    /// Mean fee of set A.
    pub mean_a: f64,
    /// Mean fee of set B.
    pub mean_b: f64,
    /// Difference in means (A - B).
    pub mean_diff: f64,
    /// Percentage difference relative to B.
    pub mean_diff_pct: f64,
    /// Median fee of set A.
    pub median_a: u64,
    /// Median fee of set B.
    pub median_b: u64,
    /// Minimum fee of set A.
    pub min_a: u64,
    /// Minimum fee of set B.
    pub min_b: u64,
    /// Maximum fee of set A.
    pub max_a: u64,
    /// Maximum fee of set B.
    pub max_b: u64,
    /// Spike count in set A.
    pub spikes_a: usize,
    /// Spike count in set B.
    pub spikes_b: usize,
    /// Overlapping time range start.
    pub overlap_start: u64,
    /// Overlapping time range end.
    pub overlap_end: u64,
}

/// Fee distribution comparator for side-by-side analysis.
pub struct Comparator;

impl Comparator {
    /// Compare two slices of fee points.
    pub fn compare(a: &[FeePoint], b: &[FeePoint], label_a: &str, label_b: &str) -> ComparisonResult {
        let mean_a = a.iter().map(|p| p.fee as f64).sum::<f64>() / a.len().max(1) as f64;
        let mean_b = b.iter().map(|p| p.fee as f64).sum::<f64>() / b.len().max(1) as f64;
        let mean_diff = mean_a - mean_b;
        let mean_diff_pct = if mean_b != 0.0 { (mean_diff / mean_b) * 100.0 } else { 0.0 };

        let mut fees_a: Vec<u64> = a.iter().map(|p| p.fee).collect();
        let mut fees_b: Vec<u64> = b.iter().map(|p| p.fee).collect();
        fees_a.sort_unstable();
        fees_b.sort_unstable();

        let median_a = fees_a.get(fees_a.len() / 2).copied().unwrap_or(0);
        let median_b = fees_b.get(fees_b.len() / 2).copied().unwrap_or(0);

        let max_ts = |pts: &[FeePoint]| pts.iter().map(|p| p.timestamp).max().unwrap_or(0);
        let min_ts = |pts: &[FeePoint]| pts.iter().map(|p| p.timestamp).min().unwrap_or(0);
        let overlap_start = min_ts(a).max(min_ts(b));
        let overlap_end = max_ts(a).min(max_ts(b));

        ComparisonResult {
            label_a: label_a.to_string(),
            label_b: label_b.to_string(),
            count_a: a.len(),
            count_b: b.len(),
            mean_a,
            mean_b,
            mean_diff,
            mean_diff_pct,
            median_a,
            median_b,
            min_a: fees_a.first().copied().unwrap_or(0),
            min_b: fees_b.first().copied().unwrap_or(0),
            max_a: fees_a.last().copied().unwrap_or(0),
            max_b: fees_b.last().copied().unwrap_or(0),
            spikes_a: a.iter().filter(|p| p.is_spike).count(),
            spikes_b: b.iter().filter(|p| p.is_spike).count(),
            overlap_start,
            overlap_end,
        }
    }

    /// Compute a similarity score [0.0, 1.0] between two fee distributions.
    pub fn similarity(a: &[FeePoint], b: &[FeePoint]) -> f64 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }
        let mean_a = a.iter().map(|p| p.fee as f64).sum::<f64>() / a.len() as f64;
        let mean_b = b.iter().map(|p| p.fee as f64).sum::<f64>() / b.len() as f64;
        let max_mean = mean_a.max(mean_b);
        if max_mean == 0.0 {
            return 1.0;
        }
        1.0 - (mean_a - mean_b).abs() / max_mean
    }

    /// Bin fee values into buckets for histogram comparison.
    pub fn histogram_bins(points: &[FeePoint], bin_count: usize) -> BTreeMap<u64, usize> {
        if points.is_empty() {
            return BTreeMap::new();
        }
        let max_fee = points.iter().map(|p| p.fee).max().unwrap();
        let bin_width = (max_fee / bin_count as u64).max(1);
        let mut bins: BTreeMap<u64, usize> = BTreeMap::new();
        for p in points {
            let bin = (p.fee / bin_width) * bin_width;
            *bins.entry(bin).or_insert(0) += 1;
        }
        bins
    }
}

impl ComparisonResult {
    /// Display the comparison as a formatted report.
    pub fn display(&self) -> String {
        format!(
            "Comparison: {} vs {}\n\
             =========================\n\
             {:>20} {:>12} {:>12}\n\
             {:->20} {:->12} {:->12}\n\
             count       {:>12} {:>12}\n\
             mean        {:>12.2} {:>12.2}\n\
             median      {:>12} {:>12}\n\
             min         {:>12} {:>12}\n\
             max         {:>12} {:>12}\n\
             spikes      {:>12} {:>12}\n\
             \n\
             mean diff:  {:.2} ({:+.2}%)\n\
             overlap:    {} - {}",
            self.label_a, self.label_b,
            "", self.label_a, self.label_b,
            "", "", "",
            self.count_a, self.count_b,
            self.mean_a, self.mean_b,
            self.median_a, self.median_b,
            self.min_a, self.min_b,
            self.max_a, self.max_b,
            self.spikes_a, self.spikes_b,
            self.mean_diff, self.mean_diff_pct,
            self.overlap_start, self.overlap_end,
        )
    }

    /// Output as JSON.
    pub fn to_json(&self) -> String {
        format!(
            r#"{{
  "label_a": "{}", "label_b": "{}",
  "count_a": {}, "count_b": {},
  "mean_a": {:.2}, "mean_b": {:.2},
  "mean_diff": {:.2}, "mean_diff_pct": {:.2},
  "median_a": {}, "median_b": {},
  "min_a": {}, "min_b": {},
  "max_a": {}, "max_b": {},
  "spikes_a": {}, "spikes_b": {},
  "overlap_start": {}, "overlap_end": {}
}}"#,
            self.label_a, self.label_b,
            self.count_a, self.count_b,
            self.mean_a, self.mean_b,
            self.mean_diff, self.mean_diff_pct,
            self.median_a, self.median_b,
            self.min_a, self.min_b,
            self.max_a, self.max_b,
            self.spikes_a, self.spikes_b,
            self.overlap_start, self.overlap_end,
        )
    }

    /// Return a human-readable verdict.
    pub fn verdict(&self) -> String {
        let abs_diff = self.mean_diff_pct.abs();
        if abs_diff < 5.0 {
            format!("{} and {} are similar (diff < 5%)", self.label_a, self.label_b)
        } else if abs_diff < 20.0 {
            format!(
                "{} is {:.0}% {} than {}",
                self.label_a,
                abs_diff,
                if self.mean_diff > 0.0 { "higher" } else { "lower" },
                self.label_b,
            )
        } else {
            format!(
                "{} is significantly {:.0}% {} than {}",
                self.label_a,
                abs_diff,
                if self.mean_diff > 0.0 { "higher" } else { "lower" },
                self.label_b,
            )
        }
    }
}

/// Arguments for the `compare` subcommand.
pub struct CompareArgs {
    /// Label for the first data set.
    pub label_a: String,
    /// Label for the second data set.
    pub label_b: String,
    /// Output as JSON.
    pub json: bool,
    /// Show histogram comparison.
    pub histogram: bool,
    /// Number of histogram bins.
    pub bins: usize,
}

impl Default for CompareArgs {
    fn default() -> Self {
        Self {
            label_a: "A".into(),
            label_b: "B".into(),
            json: false,
            histogram: false,
            bins: 10,
        }
    }
}

impl CompareArgs {
    /// Run the compare subcommand with two data sets.
    pub fn run(&self, a: &[FeePoint], b: &[FeePoint]) {
        let result = Comparator::compare(a, b, &self.label_a, &self.label_b);
        if self.json {
            println!("{}", result.to_json());
        } else {
            println!("{}", result.display());
            println!("{}", result.verdict());
        }
        if self.histogram {
            let hist_a = Comparator::histogram_bins(a, self.bins);
            let hist_b = Comparator::histogram_bins(b, self.bins);
            println!("\nHistogram bins ({}):", self.label_a);
            for (bin, count) in &hist_a {
                println!("  {}: {}", bin, "*".repeat(*count));
            }
            println!("\nHistogram bins ({}):", self.label_b);
            for (bin, count) in &hist_b {
                println!("  {}: {}", bin, "*".repeat(*count));
            }
        }
    }

    /// Validate that both data sets are non-empty.
    pub fn validate_data(a: &[FeePoint], b: &[FeePoint]) -> Result<(), String> {
        if a.is_empty() {
            return Err("first data set is empty".into());
        }
        if b.is_empty() {
            return Err("second data set is empty".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::fee_model::FeePoint;

    fn dataset_a() -> Vec<FeePoint> {
        vec![
            FeePoint { timestamp: 0, fee: 100, ledger: 1, is_spike: false },
            FeePoint { timestamp: 100, fee: 110, ledger: 2, is_spike: false },
            FeePoint { timestamp: 200, fee: 105, ledger: 3, is_spike: false },
            FeePoint { timestamp: 300, fee: 500, ledger: 4, is_spike: true },
        ]
    }

    fn dataset_b() -> Vec<FeePoint> {
        vec![
            FeePoint { timestamp: 0, fee: 200, ledger: 1, is_spike: false },
            FeePoint { timestamp: 100, fee: 210, ledger: 2, is_spike: false },
            FeePoint { timestamp: 200, fee: 205, ledger: 3, is_spike: false },
            FeePoint { timestamp: 300, fee: 205, ledger: 4, is_spike: false },
            FeePoint { timestamp: 400, fee: 600, ledger: 5, is_spike: true },
        ]
    }

    #[test]
    fn compare_returns_expected_fields() {
        let r = Comparator::compare(&dataset_a(), &dataset_b(), "test-a", "test-b");
        assert_eq!(r.label_a, "test-a");
        assert_eq!(r.count_a, 4);
        assert_eq!(r.count_b, 5);
    }

    #[test]
    fn compare_mean_diff_is_calculated() {
        let r = Comparator::compare(&dataset_a(), &dataset_b(), "a", "b");
        assert!(r.mean_diff < 0.0); // dataset_a has lower fees
    }

    #[test]
    fn similarity_returns_one_for_identical() {
        let data = dataset_a();
        let sim = Comparator::similarity(&data, &data);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn similarity_returns_zero_for_empty() {
        let sim = Comparator::similarity(&[], &dataset_b());
        assert!((sim - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_bins_partitions_data() {
        let bins = Comparator::histogram_bins(&dataset_a(), 5);
        assert!(!bins.is_empty());
    }

    #[test]
    fn comparison_result_json_is_valid() {
        let r = Comparator::compare(&dataset_a(), &dataset_b(), "a", "b");
        let json = r.to_json();
        assert!(json.contains("mean_diff"));
    }

    #[test]
    fn comparison_verdict_detects_similar() {
        let a = vec![FeePoint { timestamp: 0, fee: 100, ledger: 1, is_spike: false }];
        let b = vec![FeePoint { timestamp: 1, fee: 102, ledger: 2, is_spike: false }];
        let r = Comparator::compare(&a, &b, "a", "b");
        assert!(r.verdict().contains("similar"));
    }

    #[test]
    fn validate_data_rejects_empty() {
        assert!(CompareArgs::validate_data(&[], &dataset_b()).is_err());
    }

    #[test]
    fn compare_args_default() {
        let args = CompareArgs::default();
        assert_eq!(args.label_a, "A");
        assert_eq!(args.bins, 10);
    }
}
