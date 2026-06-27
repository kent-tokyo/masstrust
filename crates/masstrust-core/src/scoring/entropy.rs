use crate::types::QueryRanking;

pub fn score(ranking: &QueryRanking) -> Option<f64> {
    let n = ranking.candidates.len();
    if n == 1 {
        return Some(1.0);
    }
    let probs: Option<Vec<f64>> = ranking.candidates.iter().map(|c| c.probability).collect();
    let probs = probs?;

    let h: f64 = probs
        .iter()
        .map(|&p| if p == 0.0 { 0.0 } else { -p * p.ln() })
        .sum();
    let h_max = (n as f64).ln();
    let confidence = 1.0 - h / h_max;
    if confidence.is_nan() || confidence.is_infinite() {
        None
    } else {
        Some(confidence)
    }
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
                formula: None,
                is_correct: None,
            })
            .collect();
        QueryRanking {
            query_id: "q".into(),
            candidates,
        }
    }

    #[test]
    fn test_single_candidate() {
        let r = make_ranking(&[Some(1.0)]);
        assert_eq!(score(&r), Some(1.0));
    }

    #[test]
    fn test_uniform_distribution_low_confidence() {
        // uniform over 4 → max entropy → confidence near 0
        let r = make_ranking(&[Some(0.25), Some(0.25), Some(0.25), Some(0.25)]);
        let c = score(&r).unwrap();
        assert!((c - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_concentrated_distribution_high_confidence() {
        let r = make_ranking(&[Some(0.9), Some(0.05), Some(0.05)]);
        let c = score(&r).unwrap();
        assert!(c > 0.5);
    }

    #[test]
    fn test_missing_probability_returns_none() {
        let r = make_ranking(&[Some(0.7), None]);
        assert_eq!(score(&r), None);
    }

    #[test]
    fn test_zero_probability_handled() {
        let r = make_ranking(&[Some(1.0), Some(0.0)]);
        let c = score(&r);
        assert!(c.is_some());
    }
}
