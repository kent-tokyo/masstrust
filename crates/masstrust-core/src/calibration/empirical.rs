use crate::types::RiskCoverageRow;

/// Select the threshold that maximises coverage while keeping **observed risk ≤ `target`**.
///
/// Returns the confidence threshold, or `None` if no row in `curve` satisfies the
/// target error rate.
///
/// # Example
///
/// ```
/// use masstrust_core::{calibration::calibrate_empirical, RiskCoverageRow};
///
/// let curve = vec![
///     RiskCoverageRow { threshold: 0.9, accepted: 1, total: 4, coverage: 0.25, errors: 0, risk: Some(0.0) },
///     RiskCoverageRow { threshold: 0.5, accepted: 2, total: 4, coverage: 0.50, errors: 0, risk: Some(0.0) },
///     RiskCoverageRow { threshold: 0.1, accepted: 4, total: 4, coverage: 1.00, errors: 1, risk: Some(0.25) },
/// ];
/// // With target 0.05, only the first two rows qualify; max coverage is 0.5.
/// assert_eq!(calibrate_empirical(&curve, 0.05), Some(0.5));
/// ```
pub fn calibrate(curve: &[RiskCoverageRow], target: f64) -> Option<f64> {
    curve
        .iter()
        .filter(|row| row.risk.is_some_and(|r| r <= target))
        .max_by(|a, b| a.coverage.total_cmp(&b.coverage))
        .map(|row| row.threshold)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RiskCoverageRow;

    fn row(threshold: f64, coverage: f64, risk: Option<f64>) -> RiskCoverageRow {
        RiskCoverageRow {
            threshold,
            accepted: 0,
            total: 10,
            coverage,
            errors: 0,
            risk,
        }
    }

    #[test]
    fn test_selects_max_coverage_under_target() {
        let curve = vec![
            row(0.9, 0.1, Some(0.0)),
            row(0.7, 0.4, Some(0.04)),
            row(0.5, 0.7, Some(0.06)), // exceeds target
            row(0.3, 0.9, Some(0.10)),
        ];
        // target=0.05: rows at 0.9 and 0.7 qualify; max coverage is 0.4 → threshold=0.7
        assert_eq!(calibrate(&curve, 0.05), Some(0.7));
    }

    #[test]
    fn test_none_when_no_qualifying_row() {
        let curve = vec![row(0.9, 0.1, Some(0.1)), row(0.5, 0.5, Some(0.2))];
        assert_eq!(calibrate(&curve, 0.05), None);
    }

    #[test]
    fn test_empty_curve() {
        assert_eq!(calibrate(&[], 0.05), None);
    }
}
