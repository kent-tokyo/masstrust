//! # masstrust-core
//!
//! Selective prediction, confidence calibration, and abstention for MS/MS molecular annotation.
//!
//! `masstrust-core` is the library backend for the `masstrust` CLI tool.  It accepts
//! candidate rankings produced by external MS/MS annotation tools (SIRIUS, CSI:FingerID,
//! MassSpecGym retrievers, …) and decides whether the top annotation should be trusted or
//! rejected under a user-specified error-rate target.
//!
//! ## Typical workflow
//!
//! ```no_run
//! use std::path::Path;
//! use masstrust_core::{
//!     calibration::calibrate_empirical,
//!     io::{group_by_query, read_candidates},
//!     metrics::compute_curve,
//!     policy::{apply_policy, save_policy},
//!     PolicyFile, ScoringMethod, CalibrationMethod,
//! };
//!
//! // 1. Load labeled candidates and group by query spectrum.
//! let candidates = read_candidates(Path::new("examples/labeled_candidates.csv")).unwrap();
//! let rankings   = group_by_query(candidates);
//!
//! // 2. Compute the risk-coverage curve.
//! let curve = compute_curve(&rankings, ScoringMethod::ScoreGap);
//!
//! // 3. Calibrate: find the threshold that keeps observed risk ≤ 5 %.
//! let threshold = calibrate_empirical(&curve, 0.05).unwrap_or(f64::MAX);
//!
//! // 4. Save a reusable policy.
//! let policy = PolicyFile {
//!     version: "0.1.0".into(),
//!     scoring_method: ScoringMethod::ScoreGap,
//!     threshold,
//!     target_error_rate: 0.05,
//!     calibration_method: CalibrationMethod::Empirical,
//!     confidence_level: None,
//!     created_by: "masstrust".into(),
//! };
//! save_policy(&policy, Path::new("policy.json")).unwrap();
//!
//! // 5. Apply the policy to new (unlabeled) candidates.
//! let new_candidates = read_candidates(Path::new("examples/candidates.csv")).unwrap();
//! let new_rankings   = group_by_query(new_candidates);
//! let decisions      = apply_policy(&new_rankings, &policy);
//! let accepted: Vec<_> = decisions.iter().filter(|d| d.accepted).collect();
//! ```
//!
//! ## Features
//!
//! | Feature   | Effect |
//! |-----------|--------|
//! | `parquet` | Enables Parquet input via `polars`. Files with the `.parquet` extension are auto-detected. |
//!
//! ## Scientific caveats
//!
//! `masstrust` controls **observed or bounded risk** under the chosen calibration procedure
//! on the provided validation data.  It does not guarantee clinical correctness and does not
//! claim regulatory compliance.

pub mod calibration;
pub mod error;
pub mod io;
pub mod metrics;
pub mod policy;
pub mod scoring;
pub mod types;

pub use error::MasstrustError;
pub use types::*;
