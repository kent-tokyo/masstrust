use crate::types::QueryRanking;

pub fn score(ranking: &QueryRanking) -> Option<f64> {
    let top1 = ranking.candidates.iter().min_by_key(|c| c.rank)?;
    let p = top1.probability?;
    if p.is_nan() {
        None
    } else {
        Some(p)
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
            score: 0.9,
            probability: prob,
            smiles: None,
            inchikey: None,
            formula: None,
            is_correct: None,
        }
    }

    #[test]
    fn test_returns_top1_probability() {
        let r = QueryRanking {
            query_id: "q".into(),
            candidates: vec![cand(2, Some(0.2)), cand(1, Some(0.7))],
        };
        assert_eq!(score(&r), Some(0.7));
    }

    #[test]
    fn test_none_when_probability_missing() {
        let r = QueryRanking {
            query_id: "q".into(),
            candidates: vec![cand(1, None)],
        };
        assert_eq!(score(&r), None);
    }

    #[test]
    fn test_none_when_probability_nan() {
        let r = QueryRanking {
            query_id: "q".into(),
            candidates: vec![cand(1, Some(f64::NAN))],
        };
        assert_eq!(score(&r), None);
    }
}
