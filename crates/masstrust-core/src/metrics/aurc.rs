use crate::types::RiskCoverageRow;

// ── deterministic PRNG (XorShift64) — no external deps ───────────────────
struct XorShift64(u64);
impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Compute AURC from raw `(confidence, is_correct)` observations (bootstrap use).
///
/// Observations are sorted by confidence descending; ties are grouped.
/// Implicit origin `(0, 0)` is included.  Returns `NaN` if `obs` is empty.
pub fn aurc_from_obs(obs: &[(f64, bool)]) -> f64 {
    if obs.is_empty() {
        return f64::NAN;
    }
    let mut sorted = obs.to_vec();
    sorted.sort_by(|a, b| b.0.total_cmp(&a.0));

    let total = sorted.len();
    let mut accepted = 0usize;
    let mut errors = 0usize;
    let mut prev_cov = 0.0f64;
    let mut prev_risk = 0.0f64;
    let mut area = 0.0f64;
    let mut i = 0;

    while i < total {
        let conf = sorted[i].0;
        while i < total && sorted[i].0 == conf {
            accepted += 1;
            if !sorted[i].1 {
                errors += 1;
            }
            i += 1;
        }
        let cov = accepted as f64 / total as f64;
        let risk = errors as f64 / accepted as f64;
        area += (cov - prev_cov) * (prev_risk + risk) / 2.0;
        prev_cov = cov;
        prev_risk = risk;
    }
    area
}

/// Bootstrap 95% CI for AURC using a deterministic XorShift64 RNG.
///
/// Resamples `obs` with replacement `n_bootstrap` times and returns
/// the 2.5th and 97.5th percentiles.  Returns `(NaN, NaN)` if empty.
pub fn bootstrap_aurc_ci(obs: &[(f64, bool)], n_bootstrap: usize, seed: u64) -> (f64, f64) {
    if obs.is_empty() || n_bootstrap == 0 {
        return (f64::NAN, f64::NAN);
    }
    let mut rng = XorShift64::new(seed);
    let m = obs.len();
    let mut aurcs: Vec<f64> = (0..n_bootstrap)
        .map(|_| {
            let sample: Vec<(f64, bool)> = (0..m).map(|_| obs[rng.next() as usize % m]).collect();
            aurc_from_obs(&sample)
        })
        .collect();
    aurcs.sort_by(|a, b| a.total_cmp(b));
    let lo = aurcs[((0.025 * n_bootstrap as f64) as usize).min(n_bootstrap - 1)];
    let hi = aurcs[((0.975 * n_bootstrap as f64) as usize).min(n_bootstrap - 1)];
    (lo, hi)
}

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
    if last.total == 0 || last.accepted < last.total {
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

    #[test]
    fn test_eaurc_nan_when_unscoreable_queries_present() {
        // Max reachable coverage is 0.8 (accepted < total on the last row):
        // some queries are unscoreable and never accepted at any threshold.
        let curve = vec![RiskCoverageRow {
            threshold: 0.5,
            accepted: 8,
            total: 10,
            coverage: 0.8,
            errors: 1,
            risk: Some(0.125),
        }];
        assert!(compute_eaurc(&curve).is_nan());
    }

    #[test]
    fn test_aurc_from_obs_empty() {
        assert!(aurc_from_obs(&[]).is_nan());
    }

    #[test]
    fn test_aurc_from_obs_all_correct() {
        // All correct → risk always 0 → AURC = 0
        let obs = vec![(0.9, true), (0.8, true), (0.7, true)];
        assert!((aurc_from_obs(&obs) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_aurc_from_obs_matches_compute_aurc() {
        // Two distinct confidences, 1 correct 1 incorrect
        // Sorted desc: (0.9, true), (0.7, false)
        // Row 1: cov=0.5, risk=0.0;  Row 2: cov=1.0, risk=0.5
        // Area = (0.5-0)*( 0+0)/2 + (1-0.5)*(0+0.5)/2 = 0.125
        let obs = vec![(0.7_f64, false), (0.9_f64, true)];
        let a = aurc_from_obs(&obs);
        assert!((a - 0.125).abs() < 1e-10, "got {a}");
    }

    #[test]
    fn test_bootstrap_aurc_ci_empty() {
        let (lo, hi) = bootstrap_aurc_ci(&[], 100, 42);
        assert!(lo.is_nan() && hi.is_nan());
    }

    #[test]
    fn test_bootstrap_aurc_ci_all_correct() {
        // AURC = 0 always → CI must be (0, 0)
        let obs: Vec<(f64, bool)> = (0..20).map(|i| (i as f64 / 20.0, true)).collect();
        let (lo, hi) = bootstrap_aurc_ci(&obs, 200, 1);
        assert!((lo - 0.0).abs() < 1e-10, "lo={lo}");
        assert!((hi - 0.0).abs() < 1e-10, "hi={hi}");
    }

    #[test]
    fn test_bootstrap_aurc_ci_bounds() {
        let obs = vec![(0.9, true), (0.8, false), (0.7, true), (0.6, false)];
        let (lo, hi) = bootstrap_aurc_ci(&obs, 500, 42);
        assert!(lo.is_finite() && hi.is_finite());
        assert!(lo <= hi);
    }
}
