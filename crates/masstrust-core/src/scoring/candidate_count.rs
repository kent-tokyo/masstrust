use crate::types::QueryRanking;

pub fn score(ranking: &QueryRanking) -> Option<f64> {
    let n = ranking.candidates.len();
    if n == 0 {
        return None;
    }
    Some(1.0 / n as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candidate;

    fn make_ranking(n: usize) -> QueryRanking {
        let candidates = (0..n)
            .map(|i| Candidate {
                query_id: "q".into(),
                candidate_id: format!("c{i}"),
                rank: i + 1,
                score: 1.0 - i as f64 * 0.1,
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
    fn test_one_candidate_returns_one() {
        let r = make_ranking(1);
        assert_eq!(score(&r), Some(1.0));
    }

    #[test]
    fn test_four_candidates_returns_quarter() {
        let r = make_ranking(4);
        let v = score(&r).unwrap();
        assert!((v - 0.25).abs() < 1e-10, "got {v}");
    }
}
