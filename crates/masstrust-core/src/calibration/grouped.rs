use std::collections::HashMap;

use crate::error::MasstrustError;
use crate::metrics::compute_curve;
use crate::types::{CalibrationMethod, QueryRanking, ScoringMethod};

use super::{calibrate_binomial, calibrate_crc, calibrate_empirical};

/// Result of grouped calibration: per-group thresholds plus a list of groups
/// that had no valid threshold (they will fall back to the global threshold).
#[derive(Debug, Clone)]
pub struct GroupedCalibrationResult {
    /// Per-group calibrated thresholds.
    pub thresholds: HashMap<String, f64>,
    /// Groups for which no threshold satisfied the target (will use global fallback).
    pub fallback_groups: Vec<String>,
}

/// Calibrate a separate threshold for every distinct group in `rankings`.
///
/// The group of each query is taken from the `group` field of its top-1 candidate
/// (populated via [`io::read_group_column`](crate::io::read_group_column)).
/// Queries with `group = None` are silently excluded from per-group calibration;
/// they will use the global threshold when the policy is applied.
///
/// Each group is calibrated independently using its own risk-coverage curve.
/// Groups with fewer than 2 distinct confidence values in their curve (too little
/// data to calibrate) are added to `fallback_groups`.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use masstrust_core::{
///     calibration::{calibrate_grouped, calibrate_empirical},
///     io::{group_by_query, read_candidates, read_group_column},
///     metrics::compute_curve,
///     CalibrationMethod, ScoringMethod,
/// };
///
/// let mut candidates = read_candidates(Path::new("examples/labeled_candidates_grouped.csv")).unwrap();
/// let groups = read_group_column(Path::new("examples/labeled_candidates_grouped.csv"), "adduct").unwrap();
/// for (c, g) in candidates.iter_mut().zip(groups) { c.group = g; }
///
/// let rankings = group_by_query(candidates);
/// let result = calibrate_grouped(&rankings, ScoringMethod::ScoreGap, 0.05,
///                                CalibrationMethod::Empirical, None).unwrap();
/// println!("{:?}", result.thresholds);
/// ```
pub fn calibrate_grouped(
    rankings: &[QueryRanking],
    method: ScoringMethod,
    target: f64,
    cal_method: CalibrationMethod,
    confidence_level: Option<f64>,
) -> Result<GroupedCalibrationResult, MasstrustError> {
    // Partition rankings by the group of their top-1 candidate.
    let mut by_group: HashMap<String, Vec<QueryRanking>> = HashMap::new();
    for r in rankings {
        let top1 = r.candidates.iter().min_by_key(|c| c.rank);
        if let Some(group) = top1.and_then(|c| c.group.as_ref()) {
            by_group.entry(group.clone()).or_default().push(r.clone());
        }
    }

    let mut thresholds = HashMap::new();
    let mut fallback_groups = Vec::new();

    for (group, group_rankings) in &by_group {
        let curve = compute_curve(group_rankings, method);
        let threshold_opt = calibrate_one(&curve, target, cal_method, confidence_level)?;
        match threshold_opt {
            Some(t) => {
                thresholds.insert(group.clone(), t);
            }
            None => {
                fallback_groups.push(group.clone());
            }
        }
    }

    // Sort fallback_groups for deterministic output.
    fallback_groups.sort();

    Ok(GroupedCalibrationResult {
        thresholds,
        fallback_groups,
    })
}

fn calibrate_one(
    curve: &[crate::types::RiskCoverageRow],
    target: f64,
    method: CalibrationMethod,
    confidence_level: Option<f64>,
) -> Result<Option<f64>, MasstrustError> {
    match method {
        CalibrationMethod::Empirical => Ok(calibrate_empirical(curve, target)),
        CalibrationMethod::Crc => Ok(calibrate_crc(curve, target)),
        CalibrationMethod::Binomial => {
            let level = confidence_level.unwrap_or(0.95);
            calibrate_binomial(curve, target, level)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candidate;

    fn make_ranking(
        query_id: &str,
        s1: f64,
        s2: f64,
        is_correct: bool,
        group: &str,
    ) -> QueryRanking {
        QueryRanking {
            query_id: query_id.into(),
            candidates: vec![
                Candidate {
                    query_id: query_id.into(),
                    candidate_id: "c1".into(),
                    rank: 1,
                    score: s1,
                    probability: None,
                    smiles: None,
                    inchikey: None,
                    formula: None,
                    is_correct: Some(is_correct),
                    group: Some(group.into()),
                },
                Candidate {
                    query_id: query_id.into(),
                    candidate_id: "c2".into(),
                    rank: 2,
                    score: s2,
                    probability: None,
                    smiles: None,
                    inchikey: None,
                    formula: None,
                    is_correct: Some(!is_correct),
                    group: Some(group.into()),
                },
            ],
        }
    }

    #[test]
    fn test_grouped_calibration_two_groups() {
        let rankings = vec![
            // Group A: high gaps → easy to calibrate
            make_ranking("q1", 0.9, 0.5, true, "A"),
            make_ranking("q2", 0.8, 0.5, true, "A"),
            // Group B: lower gaps → harder
            make_ranking("q3", 0.7, 0.6, false, "B"),
            make_ranking("q4", 0.6, 0.5, true, "B"),
        ];

        let result = calibrate_grouped(
            &rankings,
            ScoringMethod::ScoreGap,
            0.05,
            CalibrationMethod::Empirical,
            None,
        )
        .unwrap();

        // Group A has 2 correct queries → should find a threshold at 0.05
        assert!(
            result.thresholds.contains_key("A"),
            "Group A should calibrate"
        );
        // Group B has an incorrect query at high confidence → may fall back
        // Just verify no panic and result is deterministic
        assert!(result.thresholds.len() + result.fallback_groups.len() == 2);
    }

    #[test]
    fn test_no_group_field_excluded() {
        let mut r = make_ranking("q1", 0.9, 0.5, true, "A");
        r.candidates.iter_mut().for_each(|c| c.group = None);

        let result = calibrate_grouped(
            &[r],
            ScoringMethod::ScoreGap,
            0.05,
            CalibrationMethod::Empirical,
            None,
        )
        .unwrap();

        assert!(result.thresholds.is_empty());
        assert!(result.fallback_groups.is_empty());
    }
}
