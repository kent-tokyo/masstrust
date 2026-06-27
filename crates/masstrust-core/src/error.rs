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
    #[error("Parquet input detected but masstrust was compiled without the 'parquet' feature; recompile with --features parquet")]
    ParquetNotEnabled,
}
