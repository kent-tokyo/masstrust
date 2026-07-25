use crate::types::QueryRanking;

pub fn score(ranking: &QueryRanking) -> Option<f64> {
    if ranking.candidates.len() < 2 {
        return None;
    }
    let mut sorted: Vec<_> = ranking.candidates.iter().collect();
    sorted.sort_by_key(|c| c.rank);
    let s1 = sorted[0].score;
    let s2 = sorted[1].score;
    if s1.is_nan() || s2.is_nan() || s2 <= 0.0 {
        return None;
    }
    let r = s1 / s2;
    r.is_finite().then_some(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candidate;

    fn make_ranking(scores: &[f64]) -> QueryRanking {
        let candidates = scores
            .iter()
            .enumerate()
            .map(|(i, &s)| Candidate {
                query_id: "q".into(),
                candidate_id: format!("c{i}"),
                rank: i + 1,
                score: s,
                probability: None,
                smiles: None,
                inchikey: None,
                formula: None,
                is_correct: None,
                group: None,
            })
            .collect();
        QueryRanking { query_id: "q".into(), candidates }
    }

    #[test]
    fn test_ratio_two_candidates() {
        let r = make_ranking(&[0.9, 0.7]);
        let v = score(&r).unwrap();
        assert!((v - 0.9 / 0.7).abs() < 1e-10, "got {v}");
    }

    #[test]
    fn test_zero_denominator_returns_none() {
        let r = make_ranking(&[0.9, 0.0]);
        assert_eq!(score(&r), None);
    }

    #[test]
    fn test_single_candidate_returns_none() {
        let r = make_ranking(&[0.9]);
        assert_eq!(score(&r), None);
    }
}
