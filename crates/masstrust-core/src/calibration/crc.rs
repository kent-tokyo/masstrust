use super::empirical::calibrate as calibrate_empirical;
use crate::types::RiskCoverageRow;

/// Experimental CRC-style threshold calibration.
///
/// Applies a `1/(n+1)` finite-sample correction to the empirical target before
/// selecting the threshold, inspired by Angelopoulos et al. (2022) "Conformal
/// Risk Control" (<https://arxiv.org/abs/2208.02814>).
///
/// When assumptions hold (i.i.d. calibration set, binary 0/1 annotation loss),
/// the expected risk satisfies:
///
/// ```text
/// E[risk(λ̂)] ≤ α
/// ```
///
/// **Assumptions and limitations:**
/// - Calibration queries must be i.i.d. draws from the test distribution.
/// - Loss must be binary (correct / incorrect top-1 annotation).
/// - If the test distribution differs from the calibration set (different
///   instrument, adduct type, compound class), the bound may not transfer.
/// - Small calibration sets (n < 10) produce large corrections and may yield
///   `None` even for generous targets.
///
/// Returns `None` when `target − 1/(n+1) ≤ 0` or no row satisfies the
/// adjusted target.
///
/// # Comparison with other methods
///
/// | Method        | Behaviour |
/// |---------------|-----------|
/// | `empirical`   | `observed_risk ≤ α` — no statistical guarantee |
/// | `binomial`    | `P[risk ≤ α] ≥ confidence_level` — conservative |
/// | `crc` *(this)*| `E[risk] ≤ α` when assumptions hold — experimental |
pub fn calibrate(curve: &[RiskCoverageRow], target: f64) -> Option<f64> {
    let n = curve.last()?.total;
    let adjusted = target - 1.0 / (n as f64 + 1.0);
    if adjusted <= 0.0 {
        return None;
    }
    calibrate_empirical(curve, adjusted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RiskCoverageRow;

    fn row(threshold: f64, accepted: usize, errors: usize, total: usize) -> RiskCoverageRow {
        RiskCoverageRow {
            threshold,
            accepted,
            total,
            coverage: accepted as f64 / total as f64,
            errors,
            risk: if accepted > 0 {
                Some(errors as f64 / accepted as f64)
            } else {
                None
            },
        }
    }

    #[test]
    fn test_crc_more_conservative_than_empirical() {
        // n=4, correction = 1/5 = 0.20
        // target=0.25, adjusted=0.05 → only rows with risk ≤ 0.05 qualify
        let curve = vec![
            row(0.9, 1, 0, 4), // risk=0.00 ✓ (adjusted)
            row(0.5, 2, 0, 4), // risk=0.00 ✓ (adjusted)
            row(0.1, 4, 1, 4), // risk=0.25 ✗ (adjusted), ✓ (empirical)
        ];
        let crc = calibrate(&curve, 0.25);
        assert_eq!(crc, Some(0.5)); // more conservative than empirical (0.1)
    }

    #[test]
    fn test_correction_exceeds_target_returns_none() {
        // n=4, correction=0.20, target=0.05, adjusted=-0.15 → None
        let curve = vec![row(0.9, 1, 0, 4)];
        assert_eq!(calibrate(&curve, 0.05), None);
    }

    #[test]
    fn test_empty_curve_returns_none() {
        assert_eq!(calibrate(&[], 0.05), None);
    }

    #[test]
    fn test_large_n_approaches_empirical() {
        // n=999, correction=0.001; target=0.05, adjusted=0.049 ≈ 0.05
        let curve: Vec<_> = (1..=10)
            .map(|i| row(1.0 - i as f64 * 0.1, i * 100, 0, 999))
            .collect();
        let crc = calibrate(&curve, 0.05);
        // With 0 errors, both CRC and empirical accept max coverage
        assert!(crc.is_some());
    }
}
