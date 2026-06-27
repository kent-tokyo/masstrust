use crate::types::QueryRanking;

pub fn score(ranking: &QueryRanking) -> Option<f64> {
    if ranking.candidates.len() < 2 {
        return None;
    }
    let mut sorted: Vec<_> = ranking.candidates.iter().collect();
    sorted.sort_by_key(|c| c.rank);
    let s1 = sorted[0].score;
    let s2 = sorted[1].score;
    if s1.is_nan() || s2.is_nan() {
        None
    } else {
        Some(s1 - s2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candidate;

    fn cand(rank: usize, s: f64) -> Candidate {
        Candidate {
            query_id: "q".into(),
            candidate_id: format!("c{rank}"),
            rank,
            score: s,
            probability: None,
            smiles: None,
            inchikey: None,
            formula: None,
            is_correct: None,
        }
    }

    #[test]
    fn test_gap_three_candidates() {
        let r = QueryRanking {
            query_id: "q".into(),
            candidates: vec![cand(3, 0.5), cand(1, 0.9), cand(2, 0.7)],
        };
        assert!((score(&r).unwrap() - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_single_candidate_returns_none() {
        let r = QueryRanking {
            query_id: "q".into(),
            candidates: vec![cand(1, 0.9)],
        };
        assert_eq!(score(&r), None);
    }

    #[test]
    fn test_nan_score_returns_none() {
        let r = QueryRanking {
            query_id: "q".into(),
            candidates: vec![cand(1, f64::NAN), cand(2, 0.5)],
        };
        assert_eq!(score(&r), None);
    }
}
