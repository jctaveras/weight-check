use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Summary {
    pub count: usize,
    pub mean: f64,
    pub median: f64,
    pub p95: f64,
    pub min: f64,
    pub max: f64,
    pub stddev: f64,
}

/// Summary statistics of `values`; `None` when there is nothing to summarize.
pub fn summarize(values: &[f64]) -> Option<Summary> {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| a.total_cmp(b));

    let n = sorted.len() as f64;
    let mean = sorted.iter().sum::<f64>() / n;
    let variance = sorted.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    Some(Summary {
        count: sorted.len(),
        mean,
        median: percentile(&sorted, 50.0),
        p95: percentile(&sorted, 95.0),
        min: sorted[0],
        max: sorted[sorted.len() - 1],
        stddev: variance.sqrt(),
    })
}

/// Linear-interpolated percentile of an ascending, non-empty slice.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    let rank = (p / 100.0).clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    sorted[lo] + (sorted[hi] - sorted[lo]) * (rank - lo as f64)
}

/// Gaps between consecutive values, e.g. token arrival times → inter-token latencies.
pub fn deltas(values: &[f64]) -> Vec<f64> {
    values.windows(2).map(|w| w[1] - w[0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_of_known_values() {
        let s = summarize(&[4.0, 1.0, 3.0, 2.0, 5.0]).unwrap();
        assert_eq!(s.count, 5);
        assert_eq!(s.mean, 3.0);
        assert_eq!(s.median, 3.0);
        assert_eq!(s.min, 1.0);
        assert_eq!(s.max, 5.0);
        assert!((s.p95 - 4.8).abs() < 1e-9);
        assert!((s.stddev - 2f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn empty_and_non_finite() {
        assert!(summarize(&[]).is_none());
        assert!(summarize(&[f64::NAN]).is_none());
        assert_eq!(summarize(&[7.0]).unwrap().p95, 7.0);
    }

    #[test]
    fn deltas_between_values() {
        assert_eq!(deltas(&[10.0, 15.0, 25.0]), vec![5.0, 10.0]);
        assert!(deltas(&[1.0]).is_empty());
    }
}
