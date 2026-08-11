#[derive(Debug, thiserror::Error)]
pub enum MasstrustError {
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Missing required column: {0}")]
    MissingColumn(String),
    #[error("Insufficient candidates for method '{method}': need {need}, got {got}")]
    InsufficientCandidates {
        method: &'static str,
        need: usize,
        got: usize,
    },
    #[error("Missing probability for method '{0}'")]
    MissingProbability(&'static str),
    #[error("Unknown policy version: {0}")]
    UnknownVersion(String),
    #[error("Unsupported confidence level: {0}. Supported: 0.90, 0.95, 0.975, 0.99")]
    UnsupportedConfidenceLevel(f64),
    #[error("Empty input: no candidates")]
    EmptyInput,
    #[error("Parquet error: {0}")]
    Parquet(String),
    #[error(
        "Parquet input detected but masstrust was compiled without the 'parquet' feature; recompile with --features parquet"
    )]
    ParquetNotEnabled,
    #[cfg(feature = "risksieve")]
    #[error("risksieve backend error: {0}")]
    RiskSieve(#[from] risksieve::RiskSieveError),
    #[cfg(feature = "risksieve")]
    #[error(
        "calibration query '{query_id}' is scoreable under {method:?} but has no is_correct label — labels are never silently treated as correct or incorrect"
    )]
    MissingCalibrationLabel {
        query_id: String,
        method: crate::types::ScoringMethod,
    },
    #[cfg(feature = "risksieve")]
    #[error(
        "non-finite confidence ({value}) for query '{query_id}' under {method:?} — refusing to silently treat as unscoreable or abstain"
    )]
    NonFiniteConfidence {
        query_id: String,
        method: crate::types::ScoringMethod,
        value: f64,
    },
    #[cfg(feature = "risksieve")]
    #[error(
        "certificate selected query '{query_id}' but no is_correct label for it was found when resolving realized selective risk"
    )]
    MissingRealizedLabel { query_id: String },
    #[cfg(feature = "risksieve")]
    #[error(
        "duplicate query_id '{query_id}' in calibration data — each query must appear exactly once"
    )]
    DuplicateCalibrationQueryId { query_id: String },
    #[cfg(feature = "risksieve")]
    #[error("duplicate query_id '{query_id}' in test data — each query must appear exactly once")]
    DuplicateTestQueryId { query_id: String },
    #[cfg(feature = "risksieve")]
    #[error(
        "query '{query_id}' has no value in loss source '{column}' — precomputed losses are never silently treated as missing or zero"
    )]
    MissingLossColumn { query_id: String, column: String },
    #[cfg(feature = "risksieve")]
    #[error(
        "loss value {value} for query '{query_id}' in column '{column}' is not a finite value in [0, 1] — refusing to silently clamp or exclude"
    )]
    LossOutOfRange {
        query_id: String,
        column: String,
        value: f64,
    },
    #[cfg(feature = "risksieve")]
    #[error(
        "column '{column}' has a non-numeric value {raw:?} for query '{query_id}' — refusing to silently treat malformed data as missing"
    )]
    InvalidLossValue {
        query_id: String,
        column: String,
        raw: String,
    },
    #[cfg(feature = "risksieve")]
    #[error(
        "loss source mismatch: this certificate was computed against '{certified}', but realized risk was requested against '{requested}' — resolving realized risk under a different loss than what was certified would silently misrepresent what the certificate's guarantee is about"
    )]
    LossSourceMismatch {
        certified: String,
        requested: String,
    },
}
