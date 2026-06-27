use std::path::Path;

use masstrust_core::{
    calibration::{calibrate_binomial, calibrate_empirical},
    io, metrics, policy, ScoringMethod,
};

fn labeled_csv() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/labeled_candidates.csv"
    ))
}

fn unlabeled_csv() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/candidates.csv"
    ))
}

#[test]
fn test_full_pipeline_score_gap() {
    let candidates = io::read_candidates(labeled_csv()).unwrap();
    assert_eq!(candidates.len(), 9);

    let rankings = io::group_by_query(candidates);
    assert_eq!(rankings.len(), 4);

    let curve = metrics::compute_curve(&rankings, ScoringMethod::ScoreGap);
    assert!(!curve.is_empty());

    // Verify curve is ordered by coverage ascending
    for w in curve.windows(2) {
        assert!(w[0].coverage <= w[1].coverage);
    }

    let threshold = calibrate_empirical(&curve, 0.05);
    // With the small fixture, some threshold should be found
    // (q3 has large score gap and is correct)
    assert!(threshold.is_some());
}

#[test]
fn test_full_pipeline_max_prob() {
    let candidates = io::read_candidates(labeled_csv()).unwrap();
    let rankings = io::group_by_query(candidates);
    let curve = metrics::compute_curve(&rankings, ScoringMethod::MaxProb);
    assert!(!curve.is_empty());
}

#[test]
fn test_apply_unlabeled() {
    use masstrust_core::{CalibrationMethod, PolicyFile};

    let policy = PolicyFile {
        version: "0.1.0".into(),
        scoring_method: ScoringMethod::ScoreGap,
        threshold: 0.1,
        target_error_rate: 0.05,
        calibration_method: CalibrationMethod::Empirical,
        confidence_level: None,
        created_by: "masstrust".into(),
        group_col: None,
        group_thresholds: None,
    };

    let candidates = io::read_candidates(unlabeled_csv()).unwrap();
    let rankings = io::group_by_query(candidates);
    let decisions = policy::apply_policy(&rankings, &policy);
    assert_eq!(decisions.len(), 2); // q5, q6
                                    // q5: gap = 0.90 - 0.70 = 0.20 >= 0.1 → accepted
    assert!(
        decisions
            .iter()
            .find(|d| d.query_id == "q5")
            .unwrap()
            .accepted
    );
}

#[test]
fn test_binomial_calibration_pipeline() {
    let candidates = io::read_candidates(labeled_csv()).unwrap();
    let rankings = io::group_by_query(candidates);
    let curve = metrics::compute_curve(&rankings, ScoringMethod::ScoreGap);
    // Should not error with supported confidence level
    let result = calibrate_binomial(&curve, 0.05, 0.95);
    assert!(result.is_ok());
}
