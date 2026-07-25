pub mod apply;
pub mod batch;
pub mod calibrate;
pub mod compare;
pub mod curve;
pub mod drift;
pub mod evaluate;
pub mod validate_split;

use anyhow::bail;
use masstrust_core::{
    CalibrationMethod, RiskCoverageRow, ScoringMethod,
    calibration::{calibrate_binomial, calibrate_crc, calibrate_empirical},
};

pub fn parse_scoring_method(s: &str) -> anyhow::Result<ScoringMethod> {
    match s {
        "max-prob" => Ok(ScoringMethod::MaxProb),
        "score-gap" => Ok(ScoringMethod::ScoreGap),
        "margin" => Ok(ScoringMethod::Margin),
        "entropy" => Ok(ScoringMethod::Entropy),
        "score-ratio" => Ok(ScoringMethod::ScoreRatio),
        "topk-gap" => Ok(ScoringMethod::TopKGap),
        "effective-k" => Ok(ScoringMethod::EffectiveK),
        "candidate-count" => Ok(ScoringMethod::CandidateCount),
        other => bail!(
            "Unknown scoring method: '{}'. Valid: max-prob, score-gap, margin, entropy, \
             score-ratio, topk-gap, effective-k, candidate-count",
            other
        ),
    }
}

pub fn parse_calibration_method(s: &str) -> anyhow::Result<CalibrationMethod> {
    match s {
        "empirical" => Ok(CalibrationMethod::Empirical),
        "binomial" => Ok(CalibrationMethod::Binomial),
        "crc" => Ok(CalibrationMethod::Crc),
        other => bail!(
            "Unknown calibration method: '{}'. Valid: empirical, binomial, crc",
            other
        ),
    }
}

/// Calibrate a threshold from a pre-computed risk-coverage curve.
/// Returns `None` when no threshold satisfies the target error rate.
pub fn calibrate_curve(
    curve: &[RiskCoverageRow],
    cal_method: CalibrationMethod,
    error_rate: f64,
    confidence_level: Option<f64>,
) -> anyhow::Result<Option<f64>> {
    Ok(match cal_method {
        CalibrationMethod::Empirical => calibrate_empirical(curve, error_rate),
        CalibrationMethod::Crc => calibrate_crc(curve, error_rate),
        CalibrationMethod::Binomial => {
            let level = confidence_level.ok_or_else(|| {
                anyhow::anyhow!("--confidence-level required for binomial method")
            })?;
            calibrate_binomial(curve, error_rate, level)?
        }
    })
}
