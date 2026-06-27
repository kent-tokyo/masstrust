use crate::types::QueryRanking;

pub fn score(ranking: &QueryRanking) -> Option<f64> {
    if ranking.candidates.len() < 2 {
        return None;
    }
    let mut sorted: Vec<_> = ranking.candidates.iter().collect();
    sorted.sort_by_key(|c| c.rank);
    let p1 = sorted[0].probability?;
    let p2 = sorted[1].probability?;
    if p1.is_nan() || p2.is_nan() {
        None
    } else {
        Some(p1 - p2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candidate;

    fn cand(rank: usize, prob: Option<f64>) -> Candidate {
        Candidate {
            query_id: "q".into(),
            candidate_id: format!("c{rank}"),
            rank,
            score: 0.9 - rank as f64 * 0.1,
            probability: prob,
            smiles: None,
            inchikey: None,
            formula: None,
            is_correct: None,
            group: None,
        }
    }

    #[test]
    fn test_margin_two_candidates() {
        let r = QueryRanking {
            query_id: "q".into(),
            candidates: vec![cand(1, Some(0.7)), cand(2, Some(0.3))],
        };
        assert!((score(&r).unwrap() - 0.4).abs() < 1e-10);
    }

    #[test]
    fn test_missing_probability_returns_none() {
        let r = QueryRanking {
            query_id: "q".into(),
            candidates: vec![cand(1, Some(0.7)), cand(2, None)],
        };
        assert_eq!(score(&r), None);
    }

    #[test]
    fn test_single_candidate_returns_none() {
        let r = QueryRanking {
            query_id: "q".into(),
            candidates: vec![cand(1, Some(0.9))],
        };
        assert_eq!(score(&r), None);
    }
}
