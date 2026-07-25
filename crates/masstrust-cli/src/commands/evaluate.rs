use std::path::PathBuf;

use anyhow::bail;
use clap::Args;
use masstrust_core::{io, metrics, policy, PolicyFile, RiskCoverageRow};

#[derive(Args)]
pub struct EvaluateArgs {
    /// Input labeled candidates CSV (e.g. a held-out test set)
    pub input: PathBuf,
    /// Trust policy JSON (typically calibrated on a separate validation set)
    #[arg(long)]
    pub policy: PathBuf,
    /// Output evaluation report JSON path
    #[arg(long)]
    pub out: Option<PathBuf>,
}

pub fn run(args: EvaluateArgs) -> anyhow::Result<()> {
    let p = policy::load_policy(&args.policy)?;
    if p.group_thresholds.is_some() {
        bail!(
            "masstrust evaluate does not yet support grouped policies (group_col = {:?}); \
             re-calibrate without --group-col, or evaluate per group manually.",
            p.group_col
        );
    }

    let candidates = io::read_candidates(&args.input)?;
    let rankings = io::group_by_query(candidates);
    let row = metrics::evaluate_at_threshold(&rankings, p.scoring_method, p.threshold);

    print_evaluation_report(&row, &p);

    if let Some(out_path) = &args.out {
        io::write_json(&row, out_path)?;
        eprintln!("  report written to: {}", out_path.display());
    }

    Ok(())
}

fn print_evaluation_report(row: &RiskCoverageRow, policy: &PolicyFile) {
    let score_method = format!("{:?}", policy.scoring_method);
    eprintln!("Evaluation result ({score_method}, fixed threshold from policy):");
    eprintln!("  threshold:          {:.6}", row.threshold);
    let pct = row.coverage * 100.0;
    eprintln!(
        "  coverage:           {:.4}  ({}/{} queries accepted, {pct:.1}%)",
        row.coverage, row.accepted, row.total
    );
    match row.risk {
        Some(risk) => eprintln!(
            "  observed risk:      {:.4}  ({}/{} errors)  (target was {:.4})",
            risk, row.errors, row.accepted, policy.target_error_rate
        ),
        None => eprintln!("  observed risk:      n/a  (0 accepted)"),
    }
}
