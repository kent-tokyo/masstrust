use std::path::PathBuf;

use clap::Args;
use masstrust_core::{io, metrics, scoring::compute_confidence};

use super::parse_scoring_method;

#[derive(Args)]
pub struct DriftArgs {
    /// Labeled calibration candidates CSV
    #[arg(long)]
    pub calibration: PathBuf,
    /// New (unlabeled) candidates CSV to compare against calibration
    #[arg(long)]
    pub new: PathBuf,
    /// Scoring method: max-prob, score-gap, margin, entropy, etc.
    #[arg(long)]
    pub score: String,
    /// Output JSON report path
    #[arg(long)]
    pub out: PathBuf,
}

pub fn run(args: DriftArgs) -> anyhow::Result<()> {
    let scoring_method = parse_scoring_method(&args.score)?;

    let cal_candidates = io::read_candidates(&args.calibration)?;
    let new_candidates = io::read_candidates(&args.new)?;

    let n_cal_rows = cal_candidates.len();
    let n_new_rows = new_candidates.len();

    let cal_rankings = io::group_by_query(cal_candidates);
    let new_rankings = io::group_by_query(new_candidates);

    let n_cal = cal_rankings.len();
    let n_new = new_rankings.len();

    // candidate_count_mean = total candidate rows / unique queries
    let mean_cal = if n_cal > 0 {
        n_cal_rows as f64 / n_cal as f64
    } else {
        0.0
    };
    let mean_new = if n_new > 0 {
        n_new_rows as f64 / n_new as f64
    } else {
        0.0
    };

    // Confidence values: exclude None (unscorable queries)
    let cal_confs: Vec<f64> = cal_rankings
        .iter()
        .filter_map(|r| compute_confidence(r, scoring_method))
        .collect();
    let new_confs: Vec<f64> = new_rankings
        .iter()
        .filter_map(|r| compute_confidence(r, scoring_method))
        .collect();

    let ks = metrics::ks_statistic(&cal_confs, &new_confs);
    let report = metrics::DriftReport::new(ks, n_cal, n_new, mean_cal, mean_new);

    io::write_json(&report, &args.out)?;
    eprintln!(
        "drift: {} (KS={:.3}, n_cal={n_cal}, n_new={n_new})",
        report.warning, ks
    );
    Ok(())
}
