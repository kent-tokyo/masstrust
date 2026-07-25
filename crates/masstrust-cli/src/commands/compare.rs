use std::path::PathBuf;

use clap::Args;
use masstrust_core::{io, metrics};
use serde::Serialize;

use super::{calibrate_curve, parse_calibration_method, parse_scoring_method};

#[derive(Args)]
pub struct CompareArgs {
    /// Input labeled candidates CSV
    pub input: PathBuf,
    /// Comma-separated scoring methods (e.g. score-gap,max-prob,effective-k)
    #[arg(long)]
    pub scores: String,
    /// Target error rate (e.g. 0.05)
    #[arg(long)]
    pub error_rate: f64,
    /// Calibration method: empirical, binomial, or crc
    #[arg(long, default_value = "empirical")]
    pub method: String,
    /// Confidence level for binomial (e.g. 0.95)
    #[arg(long)]
    pub confidence_level: Option<f64>,
    /// Output comparison CSV path
    #[arg(long)]
    pub out: PathBuf,
    /// Bootstrap resamples for AURC CI (0 = off)
    #[arg(long, default_value_t = 0)]
    pub bootstrap: usize,
}

#[derive(Serialize)]
struct CompareRow {
    method: String,
    threshold: Option<f64>,
    accepted: Option<usize>,
    total: Option<usize>,
    coverage: Option<f64>,
    errors: Option<usize>,
    risk: Option<f64>,
    aurc: Option<f64>,
    eaurc: Option<f64>,
    aurc_ci_lo: Option<f64>,
    aurc_ci_hi: Option<f64>,
}

pub fn run(args: CompareArgs) -> anyhow::Result<()> {
    let cal_method = parse_calibration_method(&args.method)?;
    let candidates = io::read_candidates(&args.input)?;
    let rankings = io::group_by_query(candidates);

    let mut rows: Vec<CompareRow> = Vec::new();

    for token in args.scores.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let scoring_method = parse_scoring_method(token)?;
        let curve = metrics::compute_curve(&rankings, scoring_method);

        let aurc_val = metrics::compute_aurc(&curve);
        let eaurc_val = metrics::compute_eaurc(&curve);

        let (ci_lo, ci_hi) = if args.bootstrap > 0 && !curve.is_empty() {
            let obs = metrics::obs_from_rankings(&rankings, scoring_method);
            metrics::bootstrap_aurc_ci(&obs, args.bootstrap, 42)
        } else {
            (f64::NAN, f64::NAN)
        };

        let threshold_opt =
            calibrate_curve(&curve, cal_method, args.error_rate, args.confidence_level)?;
        let curve_row = threshold_opt
            .and_then(|t| curve.iter().find(|r| r.threshold == t))
            .cloned();

        rows.push(CompareRow {
            method: token.to_string(),
            threshold: threshold_opt,
            accepted: curve_row.as_ref().map(|r| r.accepted),
            total: curve_row.as_ref().map(|r| r.total),
            coverage: curve_row.as_ref().map(|r| r.coverage),
            errors: curve_row.as_ref().map(|r| r.errors),
            risk: curve_row.as_ref().and_then(|r| r.risk),
            aurc: aurc_val.is_finite().then_some(aurc_val),
            eaurc: eaurc_val.is_finite().then_some(eaurc_val),
            aurc_ci_lo: ci_lo.is_finite().then_some(ci_lo),
            aurc_ci_hi: ci_hi.is_finite().then_some(ci_hi),
        });
    }

    io::write_csv(&rows, &args.out)?;
    eprintln!("Wrote {} rows to {}", rows.len(), args.out.display());
    Ok(())
}
