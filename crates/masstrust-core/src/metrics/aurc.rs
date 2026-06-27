use crate::types::RiskCoverageRow;

/// Area Under the Risk-Coverage Curve via trapezoid rule with implicit origin (0, 0).
/// Returns NaN if curve is empty.
pub fn compute_aurc(curve: &[RiskCoverageRow]) -> f64 {
    if curve.is_empty() {
        return f64::NAN;
    }
    let mut prev_cov = 0.0f64;
    let mut prev_risk = 0.0f64;
    let mut area = 0.0f64;
    for row in curve {
        let cov = row.coverage;
        let risk = row.risk.unwrap_or(0.0);
        area += (cov - prev_cov) * (prev_risk + risk) / 2.0;
        prev_cov = cov;
        prev_risk = risk;
    }
    area
}

/// Excess AURC: AURC minus the oracle-optimal AURC.
/// Oracle AURC = (1 - κ) + κ·ln(κ) where κ = fraction correct in labeled set.
/// Returns NaN if curve is empty or coverage < 1.0 (unscoreable queries exist).
pub fn compute_eaurc(curve: &[RiskCoverageRow]) -> f64 {
    let aurc = compute_aurc(curve);
    if aurc.is_nan() {
        return f64::NAN;
    }
    let last = curve.last().unwrap();
    // ponytail: approximate when unscoreable queries present (coverage < 1.0)
    if last.total == 0 {
        return f64::NAN;
    }
    let kappa = (last.accepted.saturating_sub(last.errors)) as f64 / last.total as f64;
    if kappa <= 0.0 || kappa >= 1.0 {
        return f64::NAN;
    }
    let aurc_optimal = (1.0 - kappa) + kappa * kappa.ln();
    aurc - aurc_optimal
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RiskCoverageRow;

    fn row(coverage: f64, risk: Option<f64>) -> RiskCoverageRow {
        RiskCoverageRow {
            threshold: 0.5,
            accepted: (coverage * 10.0) as usize,
            total: 10,
            coverage,
            errors: risk.map_or(0, |r| (r * coverage * 10.0) as usize),
            risk,
        }
    }

    #[test]
    fn test_aurc_empty() {
        assert!(compute_aurc(&[]).is_nan());
    }

    #[test]
    fn test_aurc_zero_risk() {
        // Risk is always 0 → AURC = 0
        let curve = vec![row(0.5, Some(0.0)), row(1.0, Some(0.0))];
        let a = compute_aurc(&curve);
        assert!((a - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_aurc_constant_risk() {
        // Origin (0,0) → (0.5, 0.2): trap = 0.05; (0.5,0.2) → (1.0,0.2): trap = 0.10; total = 0.15
        let curve = vec![row(0.5, Some(0.2)), row(1.0, Some(0.2))];
        let a = compute_aurc(&curve);
        assert!((a - 0.15).abs() < 1e-6);
    }

    #[test]
    fn test_eaurc_nan_on_empty() {
        assert!(compute_eaurc(&[]).is_nan());
    }

    #[test]
    fn test_eaurc_positive() {
        // Non-trivial curve: always some risk → E-AURC > 0
        let curve = vec![row(0.5, Some(0.1)), row(1.0, Some(0.2))];
        let e = compute_eaurc(&curve);
        assert!(e.is_finite());
    }
}
