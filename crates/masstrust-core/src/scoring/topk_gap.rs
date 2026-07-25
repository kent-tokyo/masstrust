use crate::types::QueryRanking;

pub fn score(ranking: &QueryRanking) -> Option<f64> {
    if ranking.candidates.len() < 2 {
        return None;
    }
    let mut sorted: Vec<_> = ranking.candidates.iter().collect();
    sorted.sort_by_key(|c| c.rank);
    let s1 = sorted[0].score;
    if s1.is_nan() {
        return None;
    }
    // mean of ranks 2..=min(k,5) — at least 1 element guaranteed (len >= 2)
    let k = sorted.len().min(5);
    let rest = &sorted[1..k];
    if rest.iter().any(|c| c.score.is_nan()) {
        return None;
    }
    let mean = rest.iter().map(|c| c.score).sum::<f64>() / rest.len() as f64;
    Some(s1 - mean)
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
    fn test_two_candidates_same_as_score_gap() {
        let r = make_ranking(&[0.9, 0.7]);
        let v = score(&r).unwrap();
        assert!((v - 0.2).abs() < 1e-10, "got {v}");
    }

    #[test]
    fn test_six_candidates_uses_k5() {
        // s1=1.0, mean(0.8,0.6,0.4,0.2)=0.5 → gap=0.5
        let r = make_ranking(&[1.0, 0.8, 0.6, 0.4, 0.2, 0.0]);
        let v = score(&r).unwrap();
        assert!((v - 0.5).abs() < 1e-10, "got {v}");
    }

    #[test]
    fn test_single_candidate_returns_none() {
        let r = make_ranking(&[0.9]);
        assert_eq!(score(&r), None);
    }
}
