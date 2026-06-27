mod entropy;
mod margin;
mod max_prob;
mod score_gap;

use crate::types::{QueryRanking, ScoringMethod};

/// Compute a confidence score for the top-ranked candidate of `ranking`.
///
/// Returns `None` when the method cannot produce a valid score (e.g. only one
/// candidate is available for [`ScoringMethod::ScoreGap`], or probability values
/// are missing for [`ScoringMethod::MaxProb`]).  A `None` result means the query
/// will always be abstained regardless of the threshold.
///
/// # Example
///
/// ```
/// use masstrust_core::{scoring::compute_confidence, QueryRanking, Candidate, ScoringMethod};
///
/// let ranking = QueryRanking {
///     query_id: "q1".into(),
///     candidates: vec![
///         Candidate { query_id: "q1".into(), candidate_id: "c1".into(), rank: 1,
///                     score: 0.9, probability: None, smiles: None,
///                     inchikey: None, formula: None, is_correct: None, group: None },
///         Candidate { query_id: "q1".into(), candidate_id: "c2".into(), rank: 2,
///                     score: 0.7, probability: None, smiles: None,
///                     inchikey: None, formula: None, is_correct: None, group: None },
///     ],
/// };
/// let confidence = compute_confidence(&ranking, ScoringMethod::ScoreGap);
/// assert!((confidence.unwrap() - 0.2).abs() < 1e-10);
/// ```
pub fn compute_confidence(ranking: &QueryRanking, method: ScoringMethod) -> Option<f64> {
    match method {
        ScoringMethod::MaxProb => max_prob::score(ranking),
        ScoringMethod::ScoreGap => score_gap::score(ranking),
        ScoringMethod::Margin => margin::score(ranking),
        ScoringMethod::Entropy => entropy::score(ranking),
    }
}
