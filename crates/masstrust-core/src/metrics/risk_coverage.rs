use crate::scoring::compute_confidence;
use crate::types::{QueryRanking, RiskCoverageRow, ScoringMethod};

/// Extract `(confidence, is_correct)` observations for scoreable labeled queries.
///
/// Queries without a label or without a computable confidence are excluded.
/// Used as input to [`crate::metrics::bootstrap_aurc_ci`].
pub fn obs_from_rankings(rankings: &[QueryRanking], method: ScoringMethod) -> Vec<(f64, bool)> {
    rankings
        .iter()
        .filter_map(|r| {
            let top1 = r.candidates.iter().min_by_key(|c| c.rank)?;
            let is_correct = top1.is_correct?;
            let conf = compute_confidence(r, method)?;
            Some((conf, is_correct))
        })
        .collect()
}

/// Compute the risk-coverage curve for a set of labeled query rankings.
///
/// Queries without an `is_correct` label are excluded from the curve.
/// Queries that cannot be scored by `method` (e.g. only one candidate for
/// [`ScoringMethod::ScoreGap`]) count toward `total` (the coverage denominator)
/// but are never accepted at any threshold.
///
/// Rows are emitted in order of **increasing coverage** (one row per distinct
/// confidence value).  `risk` is `None` for rows where `accepted == 0`.
pub fn compute_curve(rankings: &[QueryRanking], method: ScoringMethod) -> Vec<RiskCoverageRow> {
    // Collect (confidence, is_correct) for top-1 of each labeled query
    let mut entries: Vec<(Option<f64>, bool)> = rankings
        .iter()
        .filter_map(|r| {
            let top1 = r.candidates.iter().min_by_key(|c| c.rank)?;
            let is_correct = top1.is_correct?;
            let confidence = compute_confidence(r, method);
            Some((confidence, is_correct))
        })
        .collect();

    let total = entries.len();
    if total == 0 {
        return vec![];
    }

    // Sort by confidence descending; None sorts to end (never accepted)
    entries.sort_by(|a, b| match (&a.0, &b.0) {
        (Some(ca), Some(cb)) => cb.total_cmp(ca),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    let mut rows = Vec::new();
    let mut accepted = 0usize;
    let mut errors = 0usize;
    let mut i = 0;

    while i < entries.len() {
        let Some(conf) = entries[i].0 else {
            break; // all remaining have None confidence, skip
        };
        // Consume all entries with the same confidence value
        let mut j = i;
        while j < entries.len() && (entries[j].0 == Some(conf)) {
            accepted += 1;
            if !entries[j].1 {
                errors += 1;
            }
            j += 1;
        }
        let coverage = accepted as f64 / total as f64;
        let risk = if accepted > 0 {
            Some(errors as f64 / accepted as f64)
        } else {
            None
        };
        rows.push(RiskCoverageRow {
            threshold: conf,
            accepted,
            total,
            coverage,
            errors,
            risk,
        });
        i = j;
    }

    rows
}

/// Apply a single, externally-supplied confidence threshold to labeled query rankings and
/// report the coverage/risk actually achieved.
///
/// Unlike [`compute_curve`], this does not search over thresholds — it evaluates exactly one
/// (typically calibrated on a separate validation set), so calibration and evaluation never
/// share the same queries. Acceptance uses the same rule as
/// [`crate::policy::apply_policy`]: `confidence.is_finite() && confidence >= threshold`.
///
/// Queries without an `is_correct` label are excluded, matching [`compute_curve`].
pub fn evaluate_at_threshold(
    rankings: &[QueryRanking],
    method: ScoringMethod,
    threshold: f64,
) -> RiskCoverageRow {
    let entries = labeled_entries(rankings, method);
    evaluate_entries(&entries, threshold)
}

/// Build `(confidence, is_correct)` pairs for every labeled query (including unscoreable
/// ones, whose confidence is `None` — they count toward `total` but are never accepted).
fn labeled_entries(rankings: &[QueryRanking], method: ScoringMethod) -> Vec<(Option<f64>, bool)> {
    rankings
        .iter()
        .filter_map(|r| {
            let top1 = r.candidates.iter().min_by_key(|c| c.rank)?;
            let is_correct = top1.is_correct?;
            Some((compute_confidence(r, method), is_correct))
        })
        .collect()
}

/// Apply `threshold` to a pre-built set of `(confidence, is_correct)` entries.
fn evaluate_entries(entries: &[(Option<f64>, bool)], threshold: f64) -> RiskCoverageRow {
    let total = entries.len();
    let mut accepted = 0usize;
    let mut errors = 0usize;
    for &(confidence, is_correct) in entries {
        if confidence.is_some_and(|c| c.is_finite() && c >= threshold) {
            accepted += 1;
            if !is_correct {
                errors += 1;
            }
        }
    }

    RiskCoverageRow {
        threshold,
        accepted,
        total,
        coverage: if total > 0 {
            accepted as f64 / total as f64
        } else {
            0.0
        },
        errors,
        risk: if accepted > 0 {
            Some(errors as f64 / accepted as f64)
        } else {
            None
        },
    }
}

/// Bootstrap 95% CIs for the coverage and risk that [`evaluate_at_threshold`] would report,
/// by resampling labeled queries with replacement.
///
/// Returns `(coverage_ci_lo, coverage_ci_hi, risk_ci_lo, risk_ci_hi, risk_ci_n)`. `risk_ci_n`
/// is how many of the `n_bootstrap` resamples had at least one accepted query and so
/// contributed a risk value — if it's small relative to `n_bootstrap`, the risk CI is close
/// to meaningless (most resamples abstained entirely) and should be reported as such.
/// All four CI bounds are `NaN` if `rankings` has no labeled queries or `n_bootstrap == 0`.
pub fn bootstrap_evaluate_ci(
    rankings: &[QueryRanking],
    method: ScoringMethod,
    threshold: f64,
    n_bootstrap: usize,
    seed: u64,
) -> (f64, f64, f64, f64, usize) {
    let entries = labeled_entries(rankings, method);
    if entries.is_empty() || n_bootstrap == 0 {
        return (f64::NAN, f64::NAN, f64::NAN, f64::NAN, 0);
    }

    let mut rng = super::aurc::XorShift64::new(seed);
    let m = entries.len();
    let mut coverages = Vec::with_capacity(n_bootstrap);
    let mut risks = Vec::new();
    for _ in 0..n_bootstrap {
        let sample: Vec<(Option<f64>, bool)> =
            (0..m).map(|_| entries[rng.next() as usize % m]).collect();
        let row = evaluate_entries(&sample, threshold);
        coverages.push(row.coverage);
        if let Some(risk) = row.risk {
            risks.push(risk);
        }
    }

    let percentile = |v: &mut Vec<f64>, p: f64| -> f64 {
        if v.is_empty() {
            return f64::NAN;
        }
        v.sort_by(f64::total_cmp);
        v[((p * v.len() as f64) as usize).min(v.len() - 1)]
    };

    let risk_ci_n = risks.len();
    let cov_lo = percentile(&mut coverages, 0.025);
    let cov_hi = percentile(&mut coverages, 0.975);
    let risk_lo = percentile(&mut risks, 0.025);
    let risk_hi = percentile(&mut risks, 0.975);

    (cov_lo, cov_hi, risk_lo, risk_hi, risk_ci_n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candidate;

    fn make_ranking(
        query_id: &str,
        s1: f64,
        s2: f64,
        prob: Option<f64>,
        is_correct: Option<bool>,
    ) -> QueryRanking {
        QueryRanking {
            query_id: query_id.into(),
            candidates: vec![
                Candidate {
                    query_id: query_id.into(),
                    candidate_id: "c1".into(),
                    rank: 1,
                    score: s1,
                    probability: prob,
                    smiles: None,
                    inchikey: None,
                    target_inchikey: None,
                    formula: None,
                    is_correct,
                    group: None,
                },
                Candidate {
                    query_id: query_id.into(),
                    candidate_id: "c2".into(),
                    rank: 2,
                    score: s2,
                    probability: prob.map(|p| 1.0 - p),
                    smiles: None,
                    inchikey: None,
                    target_inchikey: None,
                    formula: None,
                    is_correct: is_correct.map(|b| !b),
                    group: None,
                },
            ],
        }
    }

    #[test]
    fn test_basic_curve() {
        // score_gap: q1=0.20(correct), q2=0.10(incorrect), q3=0.05(correct)
        let rankings = vec![
            make_ranking("q1", 0.90, 0.70, None, Some(true)),
            make_ranking("q2", 0.80, 0.70, None, Some(false)),
            make_ranking("q3", 0.75, 0.70, None, Some(true)),
        ];
        let rows = compute_curve(&rankings, ScoringMethod::ScoreGap);
        assert_eq!(rows.len(), 3);
        // First row: only q1 accepted (highest gap 0.20)
        assert_eq!(rows[0].accepted, 1);
        assert_eq!(rows[0].errors, 0);
        assert_eq!(rows[0].risk, Some(0.0));
        // Second row: q1 + q2 accepted
        assert_eq!(rows[1].accepted, 2);
        assert_eq!(rows[1].errors, 1);
    }

    #[test]
    fn test_no_is_correct_excluded() {
        let r = make_ranking("q1", 0.9, 0.8, None, None);
        let rows = compute_curve(&[r], ScoringMethod::ScoreGap);
        assert!(rows.is_empty());
    }

    #[test]
    fn test_tied_confidence_single_row() {
        // Both have same score_gap=0.10, so one row
        let rankings = vec![
            make_ranking("q1", 0.9, 0.8, None, Some(true)),
            make_ranking("q2", 0.7, 0.6, None, Some(false)),
        ];
        let rows = compute_curve(&rankings, ScoringMethod::ScoreGap);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].accepted, 2);
    }

    #[test]
    fn test_all_correct() {
        let rankings = vec![
            make_ranking("q1", 0.9, 0.7, None, Some(true)),
            make_ranking("q2", 0.8, 0.7, None, Some(true)),
        ];
        let rows = compute_curve(&rankings, ScoringMethod::ScoreGap);
        for row in &rows {
            assert_eq!(row.errors, 0);
            assert_eq!(row.risk, Some(0.0));
        }
    }

    #[test]
    fn test_all_incorrect() {
        let rankings = vec![
            make_ranking("q1", 0.9, 0.7, None, Some(false)),
            make_ranking("q2", 0.8, 0.7, None, Some(false)),
        ];
        let rows = compute_curve(&rankings, ScoringMethod::ScoreGap);
        assert_eq!(rows.last().unwrap().risk, Some(1.0));
    }

    #[test]
    fn test_evaluate_at_threshold_basic() {
        // gaps: q1=0.20 (correct), q2=0.10 (incorrect), q3=0.05 (correct)
        let rankings = vec![
            make_ranking("q1", 0.90, 0.70, None, Some(true)),
            make_ranking("q2", 0.80, 0.70, None, Some(false)),
            make_ranking("q3", 0.75, 0.70, None, Some(true)),
        ];
        // Threshold of 0.10 accepts q1 and q2 only (gap >= 0.10).
        let row = evaluate_at_threshold(&rankings, ScoringMethod::ScoreGap, 0.10);
        assert_eq!(row.threshold, 0.10);
        assert_eq!(row.total, 3);
        assert_eq!(row.accepted, 2);
        assert_eq!(row.errors, 1);
        assert_eq!(row.risk, Some(0.5));
    }

    #[test]
    fn test_evaluate_at_threshold_no_acceptances() {
        let rankings = vec![make_ranking("q1", 0.9, 0.7, None, Some(true))];
        let row = evaluate_at_threshold(&rankings, ScoringMethod::ScoreGap, 10.0);
        assert_eq!(row.accepted, 0);
        assert_eq!(row.risk, None);
        assert_eq!(row.coverage, 0.0);
    }

    #[test]
    fn test_evaluate_at_threshold_unscoreable_query_never_accepted() {
        // Single-candidate query: score_gap is unscoreable (None), must never be accepted
        // even at a very permissive (very low) threshold.
        let unscoreable = QueryRanking {
            query_id: "q1".into(),
            candidates: vec![Candidate {
                query_id: "q1".into(),
                candidate_id: "c1".into(),
                rank: 1,
                score: 0.9,
                probability: None,
                smiles: None,
                inchikey: None,
                target_inchikey: None,
                formula: None,
                is_correct: Some(true),
                group: None,
            }],
        };
        let scoreable = make_ranking("q2", 0.9, 0.1, None, Some(true));
        let row =
            evaluate_at_threshold(&[unscoreable, scoreable], ScoringMethod::ScoreGap, f64::MIN);
        assert_eq!(row.total, 2);
        assert_eq!(row.accepted, 1);
    }

    #[test]
    fn test_evaluate_at_threshold_excludes_unlabeled_queries() {
        let r = make_ranking("q1", 0.9, 0.8, None, None);
        let row = evaluate_at_threshold(&[r], ScoringMethod::ScoreGap, 0.0);
        assert_eq!(row.total, 0);
        assert_eq!(row.accepted, 0);
    }
}
