pub mod apply;
pub mod batch;
pub mod calibrate;
pub mod curve;

use anyhow::bail;
use masstrust_core::ScoringMethod;

pub fn parse_scoring_method(s: &str) -> anyhow::Result<ScoringMethod> {
    match s {
        "max-prob" => Ok(ScoringMethod::MaxProb),
        "score-gap" => Ok(ScoringMethod::ScoreGap),
        "margin" => Ok(ScoringMethod::Margin),
        "entropy" => Ok(ScoringMethod::Entropy),
        other => bail!(
            "Unknown scoring method: '{}'. Valid: max-prob, score-gap, margin, entropy",
            other
        ),
    }
}
