//! Optional, `risksieve`-backed batch selective-deployment certification (SCoRE-SDR).
//!
//! See `docs/risksieve-integration.md` for the full design rationale: why this is not a
//! `CalibrationMethod`, the estimand it controls, the score-orientation convention, and —
//! most importantly — why calibration and test queries are filtered to the *same*
//! scoreability predicate before anything reaches `risksieve` (filtering only the test side
//! would silently violate SCoRE-SDR's joint-exchangeability requirement).
//!
//! This module never re-derives or re-implements any of `risksieve`'s statistics. It only
//! adapts masstrust's own [`QueryRanking`]/[`ScoringMethod`] types into the calibration
//! losses and scores `risksieve::selective::sdr` expects, and maps the returned selected
//! indices back to `query_id`s.

use risksieve::selective::sdr;
use risksieve::{ClosedUnitInterval, OpenUnitInterval, RiskCertificate};

use crate::error::MasstrustError;
use crate::scoring::compute_confidence;
use crate::types::{QueryRanking, ScoringMethod};

/// Which e-value construction backs the certificate.
///
/// See `docs/risksieve-integration.md`'s "Why not `CalibrationMethod`" section: [`Coupled`]
/// is batch-composition-dependent (each test point's e-value depends on every other test
/// point's score), which is itself a reason this cannot be a reusable per-query policy,
/// independent of the reason coupled *and* independent constructions share (a certificate is
/// a decision about one specific batch, not a transferable threshold).
///
/// [`Coupled`]: Construction::Coupled
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construction {
    /// The paper's own cross-test-point-coupled construction (Equation 5.1, Theorem 5.1) —
    /// `risksieve::selective::sdr::certify`, the default entry point in that crate.
    Coupled,
    /// The per-test-point-independent construction (Equation 4.1), applied to each test
    /// point without regard to any other — `risksieve::selective::sdr::certify_independent`.
    Independent,
}

/// How many of a side's queries were scoreable under the chosen [`ScoringMethod`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreabilityCounts {
    /// Total queries on this side, before any filtering.
    pub total: usize,
    /// Queries for which `compute_confidence` returned `Some`.
    pub scoreable: usize,
}

/// A test query that never reached `risksieve` because it was unscoreable under the chosen
/// method. Always abstained; never silently dropped.
#[derive(Debug, Clone)]
pub struct UnscoreableTestQuery {
    /// The query's identifier.
    pub query_id: String,
    /// Always `"unscoreable"` today; a distinct field (rather than a bare bool) so future
    /// abstention reasons can be added without changing callers that only match on presence.
    pub reason: &'static str,
}

/// A test query that was scoreable and therefore became one entry in the index space
/// `certificate.parameter` refers into.
#[derive(Debug, Clone)]
pub struct ScoreableTestQuery {
    /// The query's identifier.
    pub query_id: String,
    /// masstrust-convention confidence (higher = more confident) — *not* the negated value
    /// actually passed to `risksieve`. See `docs/risksieve-integration.md`'s "Score
    /// orientation" section.
    pub confidence: f64,
}

/// Human-readable note on the score transform actually applied, carried alongside the
/// certificate so a report never has to re-derive or silently assume it.
pub const SCORE_ORIENTATION_NOTE: &str = "risksieve_score = -masstrust_confidence (risksieve: lower score = more trustworthy; masstrust: higher confidence = more trustworthy)";

/// Human-readable note on how `test_scores`' index space (and therefore
/// `certificate.parameter`) is ordered.
pub const QUERY_ORDERING_POLICY: &str = "query_id ascending (masstrust's existing io::group_by_query order), filtered to queries scoreable under the chosen method — the identical predicate applied to calibration and test";

/// Full result of one `certify_batch` run: the `risksieve` certificate plus everything
/// masstrust needs to map it back to `query_id`s and report on it without editorializing.
#[derive(Debug, Clone)]
pub struct BatchCertification {
    /// The certificate exactly as `risksieve` returned it. `parameter` is a sorted-ascending
    /// list of indices into `scoreable_test_queries`.
    pub certificate: RiskCertificate<Vec<usize>>,
    /// Which e-value construction produced `certificate`.
    pub construction: Construction,
    /// The confidence-scoring method used to build both sides' scores.
    pub scoring_method: ScoringMethod,
    /// See [`SCORE_ORIENTATION_NOTE`].
    pub score_orientation_note: &'static str,
    /// See [`QUERY_ORDERING_POLICY`].
    pub query_ordering_policy: &'static str,
    /// Calibration-side scoreability accounting.
    pub calibration_counts: ScoreabilityCounts,
    /// Test-side scoreability accounting.
    pub test_counts: ScoreabilityCounts,
    /// Scoreable test queries, in the exact order used as `risksieve`'s `test_scores` — this
    /// *is* the index space `certificate.parameter` refers into.
    pub scoreable_test_queries: Vec<ScoreableTestQuery>,
    /// Test queries excluded before reaching `risksieve`, always abstained.
    pub unscoreable_test_queries: Vec<UnscoreableTestQuery>,
}

impl BatchCertification {
    /// `query_id`s selected by the certificate, resolved from `certificate.parameter`.
    pub fn selected_query_ids(&self) -> Vec<&str> {
        self.certificate
            .parameter
            .iter()
            .map(|&i| self.scoreable_test_queries[i].query_id.as_str())
            .collect()
    }
}

fn top1_is_correct(ranking: &QueryRanking) -> Option<bool> {
    ranking
        .candidates
        .iter()
        .min_by_key(|c| c.rank)
        .and_then(|c| c.is_correct)
}

/// The top-1 correctness loss the spec calls for: `correct -> 0.0`, `incorrect -> 1.0`.
/// Extracted as its own function so calibration-loss and realized-loss construction can't
/// silently drift apart, and so the conversion itself is directly unit-testable.
fn correctness_loss(is_correct: bool) -> ClosedUnitInterval {
    let value = if is_correct { 0.0 } else { 1.0 };
    ClosedUnitInterval::new("loss", value).expect("0.0 and 1.0 are always valid")
}

/// Runs SCoRE-SDR batch certification (`risksieve::selective::sdr::certify` or
/// `certify_independent`, per `construction`).
///
/// `calibration` and `test` are filtered to the *same* scoreability predicate
/// (`compute_confidence` under `method` returning `Some`) before anything reaches
/// `risksieve` — see this module's and `docs/risksieve-integration.md`'s "Unscoreable
/// queries" section for why filtering only the test side would silently violate SCoRE-SDR's
/// joint-exchangeability requirement.
///
/// # Errors
///
/// - A calibration query that is scoreable but has no `is_correct` label is a hard error
///   ([`MasstrustError::MissingCalibrationLabel`]), never a silent exclusion.
/// - A non-finite confidence on either side is a hard error
///   ([`MasstrustError::NonFiniteConfidence`]) for the whole call, never a silent abstain.
/// - Invalid `alpha`/`gamma` (outside the open interval `(0, 1)`) or any error `risksieve`
///   itself returns is propagated as [`MasstrustError::RiskSieve`], unedited.
pub fn certify_batch(
    calibration: &[QueryRanking],
    test: &[QueryRanking],
    method: ScoringMethod,
    alpha: f64,
    gamma: f64,
    construction: Construction,
) -> Result<BatchCertification, MasstrustError> {
    let alpha = OpenUnitInterval::new("alpha", alpha)?;
    let gamma = OpenUnitInterval::new("gamma", gamma)?;

    let calibration_total = calibration.len();
    let mut calibration_losses = Vec::new();
    let mut calibration_scores = Vec::new();
    for ranking in calibration {
        let Some(confidence) = compute_confidence(ranking, method) else {
            // Unscoreable calibration query: excluded under the same predicate as test,
            // not a separately-defined rule. See docs/risksieve-integration.md.
            continue;
        };
        if !confidence.is_finite() {
            return Err(MasstrustError::NonFiniteConfidence {
                query_id: ranking.query_id.clone(),
                method,
                value: confidence,
            });
        }
        let Some(is_correct) = top1_is_correct(ranking) else {
            return Err(MasstrustError::MissingCalibrationLabel {
                query_id: ranking.query_id.clone(),
                method,
            });
        };
        calibration_losses.push(correctness_loss(is_correct));
        calibration_scores.push(-confidence);
    }
    let calibration_scoreable = calibration_losses.len();

    let test_total = test.len();
    let mut scoreable_test_queries = Vec::new();
    let mut unscoreable_test_queries = Vec::new();
    let mut test_scores = Vec::new();
    for ranking in test {
        match compute_confidence(ranking, method) {
            Some(confidence) => {
                if !confidence.is_finite() {
                    return Err(MasstrustError::NonFiniteConfidence {
                        query_id: ranking.query_id.clone(),
                        method,
                        value: confidence,
                    });
                }
                test_scores.push(-confidence);
                scoreable_test_queries.push(ScoreableTestQuery {
                    query_id: ranking.query_id.clone(),
                    confidence,
                });
            }
            None => {
                unscoreable_test_queries.push(UnscoreableTestQuery {
                    query_id: ranking.query_id.clone(),
                    reason: "unscoreable",
                });
            }
        }
    }
    let test_scoreable = scoreable_test_queries.len();

    let certificate = match construction {
        Construction::Coupled => sdr::certify(
            &calibration_losses,
            &calibration_scores,
            &test_scores,
            alpha,
            gamma,
        ),
        Construction::Independent => sdr::certify_independent(
            &calibration_losses,
            &calibration_scores,
            &test_scores,
            alpha,
            gamma,
        ),
    }?;

    Ok(BatchCertification {
        certificate,
        construction,
        scoring_method: method,
        score_orientation_note: SCORE_ORIENTATION_NOTE,
        query_ordering_policy: QUERY_ORDERING_POLICY,
        calibration_counts: ScoreabilityCounts {
            total: calibration_total,
            scoreable: calibration_scoreable,
        },
        test_counts: ScoreabilityCounts {
            total: test_total,
            scoreable: test_scoreable,
        },
        scoreable_test_queries,
        unscoreable_test_queries,
    })
}

/// Resolves the realized `is_correct` labels for `certification`'s selected queries, in
/// preparation for [`risksieve::selective::sdr::realized_selective_risk`] — which this
/// module deliberately does not re-implement, since its `f64`-with-no-`GuaranteeKind`
/// return type is itself the library's way of ensuring a realized risk can never be
/// mistaken for a second certificate.
///
/// `labeled` need not be the same slice passed to [`certify_batch`] as `test`, but must
/// contain a labeled entry for every selected `query_id` (a fresh, independently labeled
/// batch is the normal case: the certificate was computed against unlabeled test data, and
/// labels only became available afterward).
///
/// # Errors
///
/// A missing label for any selected query is a hard error, not a silent skip — this is a
/// post-hoc descriptive statistic, not a certificate, but it is still never allowed to
/// silently describe less than what it claims to.
pub fn resolve_realized_losses(
    certification: &BatchCertification,
    labeled: &[QueryRanking],
) -> Result<Vec<ClosedUnitInterval>, MasstrustError> {
    let by_id: std::collections::HashMap<&str, &QueryRanking> =
        labeled.iter().map(|r| (r.query_id.as_str(), r)).collect();

    certification
        .selected_query_ids()
        .into_iter()
        .map(|query_id| {
            let ranking =
                by_id
                    .get(query_id)
                    .ok_or_else(|| MasstrustError::MissingRealizedLabel {
                        query_id: query_id.to_string(),
                    })?;
            let is_correct =
                top1_is_correct(ranking).ok_or_else(|| MasstrustError::MissingRealizedLabel {
                    query_id: query_id.to_string(),
                })?;
            Ok(correctness_loss(is_correct))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Candidate;

    fn cand(
        query_id: &str,
        candidate_id: &str,
        rank: usize,
        score: f64,
        is_correct: Option<bool>,
    ) -> Candidate {
        Candidate {
            query_id: query_id.into(),
            candidate_id: candidate_id.into(),
            rank,
            score,
            probability: None,
            smiles: None,
            inchikey: None,
            formula: None,
            target_inchikey: None,
            is_correct,
            group: None,
        }
    }

    /// Two-candidate query with a score-gap and an is_correct label, in *reverse* rank order
    /// on purpose (rank 2 pushed before rank 1) so tests that care about top-1 selection
    /// can't accidentally pass just because insertion order happened to match rank order.
    fn ranking(query_id: &str, gap: f64, is_correct: bool) -> QueryRanking {
        QueryRanking {
            query_id: query_id.into(),
            candidates: vec![
                cand(query_id, "b", 2, 0.99 - gap, None),
                cand(query_id, "a", 1, 0.99, Some(is_correct)),
            ],
        }
    }

    fn extreme_batch(prefix: &str, n: usize) -> Vec<QueryRanking> {
        (0..n)
            .map(|i| {
                let correct = i % 2 == 0;
                let gap = if correct { 0.8 } else { 0.01 };
                ranking(&format!("{prefix}{i}"), gap, correct)
            })
            .collect()
    }

    // --- correctness_loss: correct/incorrect -> 0/1 loss conversion ---

    #[test]
    fn correctness_loss_maps_correct_to_zero_and_incorrect_to_one() {
        assert_eq!(correctness_loss(true).get(), 0.0);
        assert_eq!(correctness_loss(false).get(), 1.0);
    }

    // --- deterministic ordering & index<->query_id mapping ---

    #[test]
    fn scoreable_test_queries_preserve_input_order_not_a_reordering() {
        // Deliberately not query_id-sorted -- certify_batch must not introduce its own
        // sort; determinism is the caller's (io::group_by_query's) responsibility.
        let calibration = extreme_batch("c", 20);
        let test = vec![
            ranking("t3", 0.8, true),
            ranking("t1", 0.01, false),
            ranking("t2", 0.8, true),
        ];
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.1,
            Construction::Coupled,
        )
        .unwrap();
        let ids: Vec<&str> = result
            .scoreable_test_queries
            .iter()
            .map(|q| q.query_id.as_str())
            .collect();
        assert_eq!(ids, vec!["t3", "t1", "t2"]);
    }

    #[test]
    fn selected_query_ids_resolves_certificate_parameter_indices_correctly() {
        let calibration = extreme_batch("c", 40);
        let test = extreme_batch("t", 10);
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.1,
            Construction::Coupled,
        )
        .unwrap();
        let selected = result.selected_query_ids();
        // Every selected id must resolve back to the exact index risksieve returned.
        for (pos, &idx) in result.certificate.parameter.iter().enumerate() {
            assert_eq!(selected[pos], result.scoreable_test_queries[idx].query_id);
        }
    }

    // --- score orientation: the property test the spec calls out explicitly ---

    #[test]
    fn high_confidence_correct_selected_low_confidence_incorrect_not_selected() {
        let calibration = extreme_batch("c", 40);
        let test = extreme_batch("t", 10);
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.1,
            Construction::Coupled,
        )
        .unwrap();
        assert!(
            !result.certificate.parameter.is_empty(),
            "expected at least one selection on a maximally separable fixture"
        );
        for &idx in &result.certificate.parameter {
            let query_id = &result.scoreable_test_queries[idx].query_id;
            let i: usize = query_id.trim_start_matches('t').parse().unwrap();
            assert_eq!(
                i % 2,
                0,
                "selected a low-confidence, incorrect query: {query_id} \
                 (score orientation transform is backwards)"
            );
        }
    }

    #[test]
    fn score_orientation_note_is_recorded_in_the_result() {
        let calibration = extreme_batch("c", 4);
        let test = extreme_batch("t", 2);
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.3,
            0.3,
            Construction::Coupled,
        )
        .unwrap();
        assert!(result.score_orientation_note.contains('-'));
        assert!(result.query_ordering_policy.contains("query_id ascending"));
    }

    // --- score ties: masstrust-side determinism (risksieve itself proves tie symmetry) ---

    #[test]
    fn tied_confidence_scores_are_deterministic_across_repeated_calls() {
        let calibration = extreme_batch("c", 20);
        let test = vec![ranking("t0", 0.5, true), ranking("t1", 0.5, true)];
        let first = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.3,
            Construction::Coupled,
        )
        .unwrap();
        let second = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.3,
            Construction::Coupled,
        )
        .unwrap();
        assert_eq!(first.certificate.parameter, second.certificate.parameter);
        assert_eq!(
            first.certificate.diagnostics.ebh_tau_hat,
            second.certificate.diagnostics.ebh_tau_hat
        );
    }

    // --- coupled vs independent construction ---
    //
    // risksieve's own `sdr.rs` test `coupled_and_independent_can_select_different_sets`
    // proves the two constructions *can* disagree, using continuous calibration losses
    // masstrust's binary (0/1) loss encoding can't reproduce through the public
    // `certify_batch` API. What masstrust's adapter is responsible for getting right is
    // *routing* -- `Construction::Coupled` must reach `sdr::certify`, not
    // `sdr::certify_independent`, and vice versa. The oracle test below checks exactly that,
    // for both constructions, on the same fixture.

    #[test]
    fn independent_construction_runs_and_produces_a_valid_certificate() {
        let calibration = extreme_batch("c", 40);
        let test = extreme_batch("t", 10);
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.1,
            Construction::Independent,
        )
        .unwrap();
        assert_eq!(result.construction, Construction::Independent);
        assert_eq!(
            result.certificate.guarantee,
            risksieve::GuaranteeKind::SelectiveDeploymentRisk
        );
    }

    // --- unscoreable / missing-label / non-finite handling ---

    #[test]
    fn unscoreable_test_query_is_abstained_never_reaches_risksieve() {
        let calibration = extreme_batch("c", 20);
        let mut test = extreme_batch("t", 4);
        // Single-candidate query: ScoreGap needs >= 2 candidates, so this is unscoreable.
        test.push(QueryRanking {
            query_id: "unscoreable_q".into(),
            candidates: vec![cand("unscoreable_q", "only", 1, 0.9, None)],
        });
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.3,
            Construction::Coupled,
        )
        .unwrap();
        assert_eq!(result.unscoreable_test_queries.len(), 1);
        assert_eq!(result.unscoreable_test_queries[0].query_id, "unscoreable_q");
        assert_eq!(result.unscoreable_test_queries[0].reason, "unscoreable");
        assert!(
            result
                .scoreable_test_queries
                .iter()
                .all(|q| q.query_id != "unscoreable_q")
        );
    }

    #[test]
    fn unscoreable_calibration_query_is_excluded_same_predicate_as_test() {
        let mut calibration = extreme_batch("c", 20);
        // Single-candidate calibration query: unscoreable, and has no label either -- must
        // be excluded silently (not a MissingCalibrationLabel error), since exclusion happens
        // before the label check.
        calibration.push(QueryRanking {
            query_id: "unscoreable_c".into(),
            candidates: vec![cand("unscoreable_c", "only", 1, 0.9, None)],
        });
        let test = extreme_batch("t", 4);
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.3,
            Construction::Coupled,
        )
        .unwrap();
        assert_eq!(result.calibration_counts.total, 21);
        assert_eq!(result.calibration_counts.scoreable, 20);
    }

    #[test]
    fn missing_calibration_label_is_a_hard_error_not_a_silent_exclusion() {
        let mut calibration = extreme_batch("c", 20);
        // Scoreable (2 candidates, valid scores) but no is_correct label on top-1.
        calibration.push(ranking("unlabeled", 0.5, false));
        calibration.last_mut().unwrap().candidates[1].is_correct = None; // top-1 is rank 1 = index 1
        let test = extreme_batch("t", 4);
        let err = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.3,
            Construction::Coupled,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            MasstrustError::MissingCalibrationLabel { .. }
        ));
    }

    #[test]
    fn non_finite_confidence_is_a_hard_error_not_a_silent_abstain() {
        let calibration = extreme_batch("c", 20);
        let mut test = extreme_batch("t", 2);
        // MaxProb only guards against NaN, not +inf (see max_prob.rs) -- a genuinely
        // reachable non-finite confidence via a crafted probability column.
        test.push(QueryRanking {
            query_id: "inf_confidence".into(),
            candidates: vec![cand("inf_confidence", "a", 1, 0.9, Some(true))],
        });
        test.last_mut().unwrap().candidates[0].probability = Some(f64::INFINITY);
        // Calibration also needs a probability column under MaxProb.
        let mut calibration_with_prob = calibration.clone();
        for r in &mut calibration_with_prob {
            for c in &mut r.candidates {
                c.probability = Some(0.9);
            }
        }
        let err = certify_batch(
            &calibration_with_prob,
            &test,
            ScoringMethod::MaxProb,
            0.5,
            0.3,
            Construction::Coupled,
        )
        .unwrap_err();
        assert!(matches!(err, MasstrustError::NonFiniteConfidence { .. }));
    }

    // --- empty batch / zero selection ---

    #[test]
    fn empty_test_batch_is_a_valid_empty_certificate() {
        let calibration = extreme_batch("c", 20);
        let result = certify_batch(
            &calibration,
            &[],
            ScoringMethod::ScoreGap,
            0.5,
            0.3,
            Construction::Coupled,
        )
        .unwrap();
        assert_eq!(result.certificate.parameter, Vec::<usize>::new());
        assert_eq!(
            result.certificate.diagnostics.uninformative_result,
            Some(true)
        );
    }

    #[test]
    fn zero_selection_on_tight_alpha_is_a_valid_certificate_not_an_error() {
        let calibration = extreme_batch("c", 4);
        let test = extreme_batch("t", 2);
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.01,
            0.01,
            Construction::Coupled,
        )
        .unwrap();
        assert_eq!(result.certificate.parameter, Vec::<usize>::new());
        assert_eq!(
            result.certificate.guarantee,
            risksieve::GuaranteeKind::SelectiveDeploymentRisk
        );
    }

    // --- invalid alpha / gamma ---

    #[test]
    fn invalid_alpha_out_of_open_unit_interval_is_rejected() {
        let calibration = extreme_batch("c", 4);
        let test = extreme_batch("t", 2);
        let err = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            1.5,
            0.3,
            Construction::Coupled,
        )
        .unwrap_err();
        assert!(matches!(err, MasstrustError::RiskSieve(_)));
    }

    #[test]
    fn invalid_gamma_out_of_open_unit_interval_is_rejected() {
        let calibration = extreme_batch("c", 4);
        let test = extreme_batch("t", 2);
        let err = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.3,
            0.0,
            Construction::Coupled,
        )
        .unwrap_err();
        assert!(matches!(err, MasstrustError::RiskSieve(_)));
    }

    #[test]
    fn gamma_greater_than_alpha_is_valid_not_rejected() {
        // Theorem 4.2: validity holds for any gamma in (0,1), not only gamma <= alpha.
        let calibration = extreme_batch("c", 20);
        let test = extreme_batch("t", 4);
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.1,
            0.9,
            Construction::Coupled,
        );
        assert!(result.is_ok());
    }

    // --- realized risk resolution ---

    #[test]
    fn resolve_realized_losses_matches_selected_query_correctness() {
        let calibration = extreme_batch("c", 40);
        let test = extreme_batch("t", 10);
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.1,
            Construction::Coupled,
        )
        .unwrap();
        assert!(!result.certificate.parameter.is_empty());
        let losses = resolve_realized_losses(&result, &test).unwrap();
        assert_eq!(losses.len(), result.certificate.parameter.len());
        // Every selected query in the extreme fixture is genuinely correct (even index).
        assert!(losses.iter().all(|l| l.get() == 0.0));
        let risk = sdr::realized_selective_risk(&losses);
        assert_eq!(risk, 0.0);
    }

    #[test]
    fn resolve_realized_losses_errors_on_missing_label() {
        let calibration = extreme_batch("c", 40);
        let test = extreme_batch("t", 10);
        let result = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.1,
            Construction::Coupled,
        )
        .unwrap();
        assert!(!result.certificate.parameter.is_empty());
        // An unrelated, unlabeled batch: every selected query_id will be missing.
        let unlabeled: Vec<QueryRanking> = vec![];
        let err = resolve_realized_losses(&result, &unlabeled).unwrap_err();
        assert!(matches!(err, MasstrustError::MissingRealizedLabel { .. }));
    }

    #[test]
    fn realized_selective_risk_of_empty_selection_is_zero() {
        assert_eq!(sdr::realized_selective_risk(&[]), 0.0);
    }

    // --- direct oracle test: masstrust adapter vs risksieve called directly ---

    #[test]
    fn adapter_matches_risksieve_called_directly_on_the_same_fixture() {
        let calibration = extreme_batch("c", 40);
        let test = extreme_batch("t", 10);

        let via_adapter = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.1,
            Construction::Coupled,
        )
        .unwrap();

        // Rebuild the exact same calibration_losses/calibration_scores/test_scores the
        // adapter would have built, and call risksieve directly with them.
        let calibration_losses: Vec<ClosedUnitInterval> = calibration
            .iter()
            .map(|r| correctness_loss(top1_is_correct(r).unwrap()))
            .collect();
        let calibration_scores: Vec<f64> = calibration
            .iter()
            .map(|r| -compute_confidence(r, ScoringMethod::ScoreGap).unwrap())
            .collect();
        let test_scores: Vec<f64> = test
            .iter()
            .map(|r| -compute_confidence(r, ScoringMethod::ScoreGap).unwrap())
            .collect();

        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let gamma = OpenUnitInterval::new("gamma", 0.1).unwrap();
        let direct = sdr::certify(
            &calibration_losses,
            &calibration_scores,
            &test_scores,
            alpha,
            gamma,
        )
        .unwrap();

        assert_eq!(via_adapter.certificate.parameter, direct.parameter);
        assert_eq!(via_adapter.certificate.guarantee, direct.guarantee);
        assert_eq!(
            via_adapter.certificate.diagnostics.ebh_tau_hat,
            direct.diagnostics.ebh_tau_hat
        );
        assert_eq!(
            via_adapter.certificate.diagnostics.selected_count,
            direct.diagnostics.selected_count
        );
        assert_eq!(via_adapter.certificate.assumptions, direct.assumptions);
    }

    #[test]
    fn independent_construction_routes_to_certify_independent_not_certify() {
        let calibration = extreme_batch("c", 40);
        let test = extreme_batch("t", 10);

        let via_adapter = certify_batch(
            &calibration,
            &test,
            ScoringMethod::ScoreGap,
            0.5,
            0.1,
            Construction::Independent,
        )
        .unwrap();

        let calibration_losses: Vec<ClosedUnitInterval> = calibration
            .iter()
            .map(|r| correctness_loss(top1_is_correct(r).unwrap()))
            .collect();
        let calibration_scores: Vec<f64> = calibration
            .iter()
            .map(|r| -compute_confidence(r, ScoringMethod::ScoreGap).unwrap())
            .collect();
        let test_scores: Vec<f64> = test
            .iter()
            .map(|r| -compute_confidence(r, ScoringMethod::ScoreGap).unwrap())
            .collect();

        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let gamma = OpenUnitInterval::new("gamma", 0.1).unwrap();
        let direct_independent = sdr::certify_independent(
            &calibration_losses,
            &calibration_scores,
            &test_scores,
            alpha,
            gamma,
        )
        .unwrap();

        assert_eq!(
            via_adapter.certificate.parameter,
            direct_independent.parameter
        );
    }
}
