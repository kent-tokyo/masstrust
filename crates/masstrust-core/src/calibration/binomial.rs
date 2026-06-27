use crate::error::MasstrustError;
use crate::types::RiskCoverageRow;

fn z_for_confidence_level(level: f64) -> Result<f64, MasstrustError> {
    // One-sided z values
    let z = match (level * 1000.0).round() as u32 {
        900 => 1.282,
        950 => 1.645,
        975 => 1.960,
        990 => 2.326,
        _ => return Err(MasstrustError::UnsupportedConfidenceLevel(level)),
    };
    Ok(z)
}

fn wilson_upper(errors: usize, accepted: usize, z: f64) -> f64 {
    let p_hat = errors as f64 / accepted as f64;
    let n = accepted as f64;
    let z2 = z * z;
    (p_hat + z2 / (2.0 * n) + z * (p_hat * (1.0 - p_hat) / n + z2 / (4.0 * n * n)).sqrt())
        / (1.0 + z2 / n)
}

/// Select the threshold that maximises coverage while keeping the **one-sided Wilson
/// upper confidence bound on the error rate ≤ `target`**.
///
/// More conservative than [`calibrate_empirical`](crate::calibration::calibrate_empirical);
/// recommended for high-stakes settings.
///
/// `confidence_level` must be one of `0.90`, `0.95`, `0.975`, or `0.99`.
/// Returns [`MasstrustError::UnsupportedConfidenceLevel`] for any other value.
///
/// Returns the confidence threshold, or `Ok(None)` if no row satisfies the target.
pub fn calibrate(
    curve: &[RiskCoverageRow],
    target: f64,
    confidence_level: f64,
) -> Result<Option<f64>, MasstrustError> {
    let z = z_for_confidence_level(confidence_level)?;
    let result = curve
        .iter()
        .filter(|row| row.accepted > 0 && wilson_upper(row.errors, row.accepted, z) <= target)
        .max_by(|a, b| a.coverage.total_cmp(&b.coverage))
        .map(|row| row.threshold);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RiskCoverageRow;

    fn row(threshold: f64, accepted: usize, errors: usize, total: usize) -> RiskCoverageRow {
        let coverage = accepted as f64 / total as f64;
        let risk = if accepted > 0 {
            Some(errors as f64 / accepted as f64)
        } else {
            None
        };
        RiskCoverageRow {
            threshold,
            accepted,
            total,
            coverage,
            errors,
            risk,
        }
    }

    #[test]
    fn test_unsupported_confidence_level() {
        let err = calibrate(&[], 0.05, 0.80).unwrap_err();
        assert!(
            matches!(err, MasstrustError::UnsupportedConfidenceLevel(v) if (v - 0.80).abs() < 1e-10)
        );
    }

    #[test]
    fn test_conservative_threshold() {
        // 0 errors out of 10 accepted: Wilson upper bound at 0.95 ≈ 0.259
        // So target=0.05 should fail for this row
        // 0 errors out of 100 accepted: Wilson upper bound is lower → may pass
        let curve = vec![
            row(0.9, 10, 0, 100),  // upper ~ 0.259 > 0.05
            row(0.5, 100, 0, 100), // upper ~ 0.036 < 0.05 → qualifies
        ];
        let t = calibrate(&curve, 0.05, 0.95).unwrap();
        assert_eq!(t, Some(0.5));
    }

    #[test]
    fn test_none_when_nothing_qualifies() {
        let curve = vec![row(0.9, 2, 1, 10)];
        let t = calibrate(&curve, 0.05, 0.95).unwrap();
        assert_eq!(t, None);
    }

    #[test]
    fn test_accepted_zero_row_skipped() {
        let curve = vec![RiskCoverageRow {
            threshold: 0.9,
            accepted: 0,
            total: 10,
            coverage: 0.0,
            errors: 0,
            risk: None,
        }];
        let t = calibrate(&curve, 0.05, 0.95).unwrap();
        assert_eq!(t, None);
    }
}
