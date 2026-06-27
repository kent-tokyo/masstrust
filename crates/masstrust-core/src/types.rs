use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// One candidate molecular annotation for a query spectrum.
///
/// Required fields (`query_id`, `candidate_id`, `rank`, `score`) must be present in the
/// input CSV.  All other fields are optional.
#[derive(Debug, Clone, Deserialize)]
pub struct Candidate {
    /// Identifier of the query spectrum.
    pub query_id: String,
    /// Identifier of this candidate structure (e.g. InChIKey, SMILES, or an internal ID).
    pub candidate_id: String,
    /// Rank of this candidate among all candidates for the same query (1 = best).
    pub rank: usize,
    /// Raw annotation score (higher is better).
    pub score: f64,
    /// Optional posterior probability assigned to this candidate.
    pub probability: Option<f64>,
    /// Optional SMILES string.
    pub smiles: Option<String>,
    /// Optional InChIKey.
    pub inchikey: Option<String>,
    /// Optional molecular formula.
    pub formula: Option<String>,
    /// Ground-truth label used for calibration and evaluation (`true` = correct annotation).
    pub is_correct: Option<bool>,
    /// Calibration group (e.g. adduct type, instrument).  Not read from CSV automatically;
    /// populated via [`io::read_group_column`](crate::io::read_group_column).
    #[serde(skip_deserializing, default)]
    pub group: Option<String>,
}

/// All candidates for a single query spectrum, grouped together.
#[derive(Debug, Clone)]
pub struct QueryRanking {
    /// Identifier of the query spectrum.
    pub query_id: String,
    /// Candidates for this query, in arbitrary order (sorted internally as needed).
    pub candidates: Vec<Candidate>,
}

/// The trust decision made for the top-ranked candidate of a query spectrum.
#[derive(Debug, Clone, Serialize)]
pub struct AnnotationDecision {
    /// Identifier of the query spectrum.
    pub query_id: String,
    /// Identifier of the top-ranked candidate that was evaluated.
    pub candidate_id: String,
    /// Computed confidence score.  `NAN` if the scoring method could not produce a value
    /// (e.g. only one candidate available for `score-gap`).
    pub confidence: f64,
    /// Whether the annotation was accepted (`confidence >= threshold` and confidence is finite).
    pub accepted: bool,
    /// Threshold used for the accept/abstain decision (may be group-specific).
    pub threshold: f64,
    /// Name of the scoring method that produced `confidence`.
    pub method: String,
}

/// Confidence scoring method.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoringMethod {
    /// Top-1 candidate probability (`probability` column required).
    MaxProb,
    /// Score gap between rank-1 and rank-2 candidates (`score(1) − score(2)`).
    ScoreGap,
    /// Probability margin between rank-1 and rank-2 (`probability(1) − probability(2)`).
    Margin,
    /// `1 − H_normalized` where `H` is the Shannon entropy over candidate probabilities.
    /// Higher values mean higher confidence.
    Entropy,
}

/// Threshold calibration method.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalibrationMethod {
    /// Selects the threshold with maximum coverage where observed risk ≤ target.
    Empirical,
    /// Selects the threshold with maximum coverage where the one-sided Wilson upper bound
    /// on the error rate is ≤ target.  More conservative; recommended for high-stakes use.
    Binomial,
    /// Experimental CRC-style finite-sample correction (Angelopoulos et al., 2022).
    /// Tightens the empirical target by `1/(n+1)`.  When the calibration set is i.i.d.
    /// and the loss is binary 0/1, the expected error rate is controlled at `target`.
    /// See [`calibration::calibrate_crc`](crate::calibration::calibrate_crc) for caveats.
    Crc,
}

/// Runtime representation of a calibrated trust policy.
#[derive(Debug, Clone)]
pub struct TrustPolicy {
    pub scoring_method: ScoringMethod,
    pub threshold: f64,
    pub target_error_rate: f64,
    pub calibration_method: CalibrationMethod,
}

/// Serializable policy file, suitable for JSON export/import.
///
/// Load with [`policy::load_policy`](crate::policy::load_policy) and
/// apply with [`policy::apply_policy`](crate::policy::apply_policy).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyFile {
    /// Schema version.  Must be `"0.1.0"` for this release.
    pub version: String,
    pub scoring_method: ScoringMethod,
    /// Global confidence threshold (fallback when no group-specific threshold applies).
    pub threshold: f64,
    pub target_error_rate: f64,
    pub calibration_method: CalibrationMethod,
    /// Confidence level used with [`CalibrationMethod::Binomial`].  `None` for empirical/CRC.
    pub confidence_level: Option<f64>,
    pub created_by: String,
    /// CSV column used for group assignment during calibration.  `None` = no grouping.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_col: Option<String>,
    /// Per-group thresholds.  Queries whose group is not in this map use `threshold`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_thresholds: Option<HashMap<String, f64>>,
}

/// One row of a risk-coverage curve.
///
/// The curve is produced by [`metrics::compute_curve`](crate::metrics::compute_curve)
/// and can be written to CSV with [`io::write_csv`](crate::io::write_csv).
#[derive(Debug, Clone, Serialize)]
pub struct RiskCoverageRow {
    /// Confidence threshold at which this row was computed.
    pub threshold: f64,
    /// Number of queries accepted at this threshold.
    pub accepted: usize,
    /// Total number of labeled queries (denominator for coverage).
    pub total: usize,
    /// `accepted / total`.
    pub coverage: f64,
    /// Number of accepted queries whose top-1 annotation was incorrect.
    pub errors: usize,
    /// `errors / accepted`.  `None` when `accepted == 0` (avoids division by zero).
    pub risk: Option<f64>,
}
