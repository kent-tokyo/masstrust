use crate::scoring::compute_confidence;
use crate::types::{QueryRanking, RiskCoverageRow, ScoringMethod};

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
}
