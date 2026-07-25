use crate::types::QueryRanking;

pub fn score(ranking: &QueryRanking) -> Option<f64> {
    let probs: Option<Vec<f64>> = ranking.candidates.iter().map(|c| c.probability).collect();
    let probs = probs?;
    // H in nats: zero probability contributes 0 (matching entropy.rs convention)
    let h: f64 = probs
        .iter()
        .map(|&p| if p == 0.0 { 0.0 } else { -p * p.ln() })
        .sum();
    // exp(-H) = 1/effective_k; single candidate → H=0 → 1.0; uniform n → 1/n
    let result = (-h).exp();
    result.is_finite().then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candidate;

    fn make_ranking(probs: &[Option<f64>]) -> QueryRanking {
        let candidates = probs
            .iter()
            .enumerate()
            .map(|(i, &p)| Candidate {
                query_id: "q".into(),
                candidate_id: format!("c{i}"),
                rank: i + 1,
                score: 1.0 - i as f64 * 0.1,
                probability: p,
                smiles: None,
                inchikey: None,
                target_inchikey: None,
                formula: None,
                is_correct: None,
                group: None,
            })
            .collect();
        QueryRanking {
            query_id: "q".into(),
            candidates,
        }
    }

    #[test]
    fn test_single_certain_candidate() {
        let r = make_ranking(&[Some(1.0)]);
        let v = score(&r).unwrap();
        assert!((v - 1.0).abs() < 1e-10, "got {v}");
    }

    #[test]
    fn test_uniform_four_returns_quarter() {
        // H = ln(4), exp(-H) = 1/4
        let r = make_ranking(&[Some(0.25), Some(0.25), Some(0.25), Some(0.25)]);
        let v = score(&r).unwrap();
        assert!((v - 0.25).abs() < 1e-10, "got {v}");
    }

    #[test]
    fn test_missing_prob_returns_none() {
        let r = make_ranking(&[Some(0.7), None]);
        assert_eq!(score(&r), None);
    }
}
