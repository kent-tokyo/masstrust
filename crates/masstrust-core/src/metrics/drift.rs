use serde::{Deserialize, Serialize};

/// Two-sample Kolmogorov–Smirnov statistic.
///
/// Returns `NaN` if either slice is empty.
pub fn ks_statistic(a: &[f64], b: &[f64]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return f64::NAN;
    }
    let mut sa = a.to_vec();
    let mut sb = b.to_vec();
    sa.sort_by(|x, y| x.total_cmp(y));
    sb.sort_by(|x, y| x.total_cmp(y));
    let n1 = sa.len() as f64;
    let n2 = sb.len() as f64;
    let mut max_diff = 0.0f64;
    let mut i = 0usize;
    let mut j = 0usize;
    // Walk through sorted values using two-pointer merge
    while i < sa.len() || j < sb.len() {
        let next = match (sa.get(i), sb.get(j)) {
            (Some(&x), Some(&y)) => x.min(y),
            (Some(&x), None) => x,
            (None, Some(&y)) => y,
            (None, None) => break,
        };
        while i < sa.len() && sa[i] <= next {
            i += 1;
        }
        while j < sb.len() && sb[j] <= next {
            j += 1;
        }
        max_diff = max_diff.max((i as f64 / n1 - j as f64 / n2).abs());
    }
    max_diff
}

fn warning_level(ks: f64) -> &'static str {
    // NAN comparisons return false, so NaN maps to "low" safely
    if ks > 0.3 {
        "high"
    } else if ks > 0.15 {
        "medium"
    } else {
        "low"
    }
}

/// Summary of a distribution drift check between calibration and new data.
#[derive(Debug, Serialize, Deserialize)]
pub struct DriftReport {
    pub warning: String,
    pub confidence_ks: f64,
    pub n_calibration: usize,
    pub n_new: usize,
    pub candidate_count_mean_calibration: f64,
    pub candidate_count_mean_new: f64,
    pub message: String,
}

impl DriftReport {
    pub fn new(ks: f64, n_cal: usize, n_new: usize, mean_cal: f64, mean_new: f64) -> Self {
        let level = warning_level(ks);
        let message = if ks > 0.3 {
            format!(
                "Confidence distribution shifted (KS={ks:.2} > 0.3); recalibration may be needed."
            )
        } else if ks > 0.15 {
            format!("Moderate confidence shift detected (KS={ks:.2} > 0.15); monitor performance.")
        } else {
            format!("No significant distribution shift detected (KS={ks:.2}).")
        };
        Self {
            warning: level.to_string(),
            confidence_ks: ks,
            n_calibration: n_cal,
            n_new,
            candidate_count_mean_calibration: mean_cal,
            candidate_count_mean_new: mean_new,
            message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ks_identical_is_zero() {
        let v = vec![0.1, 0.5, 0.9];
        assert!((ks_statistic(&v, &v) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn ks_disjoint_is_one() {
        let a = vec![0.0, 0.1, 0.2];
        let b = vec![0.8, 0.9, 1.0];
        assert!((ks_statistic(&a, &b) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn ks_empty_is_nan() {
        assert!(ks_statistic(&[], &[1.0]).is_nan());
        assert!(ks_statistic(&[1.0], &[]).is_nan());
    }

    #[test]
    fn ks_known_case() {
        // a=[0,1] (CDFs: 0→0.5, 1→1.0), b=[0.5] (CDF: 0.5→1.0)
        // At 0.5: |0.5/1 - 1/1| = |0.5 - 1| = 0.5
        // At 1.0: |2/2 - 1/1| = |1 - 1| = 0
        let a = vec![0.0, 1.0];
        let b = vec![0.5];
        let ks = ks_statistic(&a, &b);
        assert!((ks - 0.5).abs() < 1e-10, "got {ks}");
    }
}
