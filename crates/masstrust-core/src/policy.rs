use std::path::Path;

use crate::error::MasstrustError;
use crate::io::{read_policy, write_json};
use crate::scoring::compute_confidence;
use crate::types::{AnnotationDecision, PolicyFile, QueryRanking};

/// Serialize `policy` to a pretty-printed JSON file at `path`.
pub fn save_policy(policy: &PolicyFile, path: &Path) -> Result<(), MasstrustError> {
    write_json(policy, path)
}

/// Load and validate a [`PolicyFile`] from `path`.
///
/// Returns [`MasstrustError::UnknownVersion`] if the JSON `"version"` field does not match
/// the current schema version.
pub fn load_policy(path: &Path) -> Result<PolicyFile, MasstrustError> {
    read_policy(path)
}

/// Apply `policy` to every ranking in `rankings` and return one [`AnnotationDecision`] per query.
///
/// Does **not** require `is_correct` labels; suitable for production (unlabeled) data.
///
/// Queries whose top-1 candidate cannot be scored (e.g. single-candidate queries with
/// `score-gap`) receive `confidence = NAN` and `accepted = false`.
///
/// When `policy.group_thresholds` is set, the threshold is looked up by the `group` field
/// of the top-1 candidate.  Queries with an unknown group (or `group = None`) fall back to
/// `policy.threshold`.
pub fn apply_policy(rankings: &[QueryRanking], policy: &PolicyFile) -> Vec<AnnotationDecision> {
    rankings
        .iter()
        .filter_map(|r| {
            let top1 = r.candidates.iter().min_by_key(|c| c.rank)?;
            let confidence = compute_confidence(r, policy.scoring_method).unwrap_or(f64::NAN);

            // Use group-specific threshold if available; fall back to global.
            let threshold = top1
                .group
                .as_ref()
                .and_then(|g| policy.group_thresholds.as_ref()?.get(g).copied())
                .unwrap_or(policy.threshold);

            let accepted = confidence.is_finite() && confidence >= threshold;
            Some(AnnotationDecision {
                query_id: r.query_id.clone(),
                candidate_id: top1.candidate_id.clone(),
                confidence,
                accepted,
                threshold,
                method: format!("{:?}", policy.scoring_method),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CalibrationMethod, Candidate, ScoringMethod};
    use tempfile::NamedTempFile;

    fn make_policy(threshold: f64) -> PolicyFile {
        PolicyFile {
            version: "0.1.0".into(),
            scoring_method: ScoringMethod::ScoreGap,
            threshold,
            target_error_rate: 0.05,
            calibration_method: CalibrationMethod::Empirical,
            confidence_level: None,
            created_by: "masstrust".into(),
            group_col: None,
            group_thresholds: None,
        }
    }

    fn make_ranking_two(query_id: &str, s1: f64, s2: f64) -> QueryRanking {
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
                    is_correct: None,
                    group: None,
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
                    is_correct: None,
                    group: None,
                },
            ],
        }
    }

    #[test]
    fn test_policy_json_roundtrip() {
        let policy = make_policy(0.18);
        let f = NamedTempFile::new().unwrap();
        save_policy(&policy, f.path()).unwrap();
        let loaded = load_policy(f.path()).unwrap();
        assert_eq!(loaded.threshold, 0.18);
        assert_eq!(loaded.scoring_method, ScoringMethod::ScoreGap);
    }

    #[test]
    fn test_unknown_version_error() {
        let mut policy = make_policy(0.18);
        policy.version = "9.9.9".into();
        let f = NamedTempFile::new().unwrap();
        save_policy(&policy, f.path()).unwrap();
        let err = load_policy(f.path()).unwrap_err();
        assert!(matches!(err, MasstrustError::UnknownVersion(_)));
    }

    #[test]
    fn test_apply_policy_accepted() {
        let policy = make_policy(0.1);
        let r = make_ranking_two("q1", 0.9, 0.7); // gap = 0.2 >= 0.1
        let decisions = apply_policy(&[r], &policy);
        assert!(decisions[0].accepted);
    }

    #[test]
    fn test_apply_policy_abstained() {
        let policy = make_policy(0.5);
        let r = make_ranking_two("q1", 0.9, 0.7); // gap = 0.2 < 0.5
        let decisions = apply_policy(&[r], &policy);
        assert!(!decisions[0].accepted);
    }

    #[test]
    fn test_apply_policy_unscoreable_abstains() {
        let policy = make_policy(0.1);
        // Single candidate → score_gap = None → abstain
        let r = QueryRanking {
            query_id: "q1".into(),
            candidates: vec![Candidate {
                query_id: "q1".into(),
                candidate_id: "c1".into(),
                rank: 1,
                score: 0.9,
                probability: None,
                smiles: None,
                inchikey: None,
                formula: None,
                is_correct: None,
                group: None,
            }],
        };
        let decisions = apply_policy(&[r], &policy);
        assert!(!decisions[0].accepted);
        assert!(decisions[0].confidence.is_nan());
    }
}
