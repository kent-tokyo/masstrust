use std::path::PathBuf;

use anyhow::bail;
use clap::Args;
use masstrust_core::{calibration, io, metrics, policy, PolicyFile};
use serde::Serialize;

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
    /// Bootstrap resamples for coverage/risk 95% CI (0 = off)
    #[arg(long, default_value_t = 0)]
    pub bootstrap: usize,
}

#[derive(Serialize)]
struct EvaluationReport {
    threshold: f64,
    accepted: usize,
    total: usize,
    coverage: f64,
    coverage_ci_lo: Option<f64>,
    coverage_ci_hi: Option<f64>,
    errors: usize,
    risk: Option<f64>,
    risk_ci_lo: Option<f64>,
    risk_ci_hi: Option<f64>,
    /// How many of the `--bootstrap` resamples had `accepted > 0` and so contributed a risk
    /// value. If this is small relative to the requested resample count, `risk_ci_*` is close
    /// to meaningless (most resamples abstained entirely).
    risk_ci_n: Option<usize>,
    risk_wilson_upper: Option<f64>,
    wilson_confidence_level: Option<f64>,
    target_error_rate: f64,
    /// `None` when `accepted == 0` (no risk was observed to compare against the target).
    target_risk_exceeded: Option<bool>,
    abstain_all: bool,
    abstain_reason: Option<String>,
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

    let (coverage_ci_lo, coverage_ci_hi, risk_ci_lo, risk_ci_hi, risk_ci_n) = if args.bootstrap > 0
    {
        let (cov_lo, cov_hi, risk_lo, risk_hi, n) = metrics::bootstrap_evaluate_ci(
            &rankings,
            p.scoring_method,
            p.threshold,
            args.bootstrap,
            42,
        );
        (
            cov_lo.is_finite().then_some(cov_lo),
            cov_hi.is_finite().then_some(cov_hi),
            risk_lo.is_finite().then_some(risk_lo),
            risk_hi.is_finite().then_some(risk_hi),
            Some(n),
        )
    } else {
        (None, None, None, None, None)
    };

    let wilson_confidence_level = p.confidence_level.unwrap_or(0.95);
    let risk_wilson_upper = if row.accepted > 0 {
        calibration::wilson_upper_bound(row.errors, row.accepted, wilson_confidence_level).ok()
    } else {
        None
    };

    let abstain_all = row.accepted == 0;
    let abstain_reason = abstain_all.then(|| {
        format!(
            "0/{} labeled queries met threshold {} — check `masstrust drift` between the \
             calibration and evaluation data, or whether the policy threshold is too strict \
             for this scoring method on this data.",
            row.total,
            format_threshold(row.threshold)
        )
    });

    let report = EvaluationReport {
        threshold: row.threshold,
        accepted: row.accepted,
        total: row.total,
        coverage: row.coverage,
        coverage_ci_lo,
        coverage_ci_hi,
        errors: row.errors,
        risk: row.risk,
        risk_ci_lo,
        risk_ci_hi,
        risk_ci_n,
        risk_wilson_upper,
        wilson_confidence_level: risk_wilson_upper.map(|_| wilson_confidence_level),
        target_error_rate: p.target_error_rate,
        target_risk_exceeded: row.risk.map(|r| r > p.target_error_rate),
        abstain_all,
        abstain_reason,
    };

    print_evaluation_report(&report, &p);

    if let Some(out_path) = &args.out {
        io::write_json(&report, out_path)?;
        eprintln!("  report written to: {}", out_path.display());
    }

    Ok(())
}

/// `calibrate`'s sentinel for "no threshold satisfies the target" is `f64::MAX` (see
/// commands/calibrate.rs), which prints as an unreadable ~300-digit number under `{:.6}`.
/// Mirror calibrate's own `+inf` display for that sentinel here.
fn format_threshold(threshold: f64) -> String {
    if !threshold.is_finite() || threshold == f64::MAX {
        "+inf".to_string()
    } else {
        format!("{threshold:.6}")
    }
}

fn print_evaluation_report(report: &EvaluationReport, policy: &PolicyFile) {
    let score_method = format!("{:?}", policy.scoring_method);
    eprintln!("Evaluation result ({score_method}, fixed threshold from policy):");
    eprintln!(
        "  threshold:          {}",
        format_threshold(report.threshold)
    );
    let pct = report.coverage * 100.0;
    let cov_ci = match (report.coverage_ci_lo, report.coverage_ci_hi) {
        (Some(lo), Some(hi)) => format!("  (95% CI [{lo:.4}, {hi:.4}])"),
        _ => String::new(),
    };
    eprintln!(
        "  coverage:           {:.4}  ({}/{} queries accepted, {pct:.1}%){cov_ci}",
        report.coverage, report.accepted, report.total
    );
    match report.risk {
        Some(risk) => {
            let risk_ci = match (report.risk_ci_lo, report.risk_ci_hi, report.risk_ci_n) {
                (Some(lo), Some(hi), Some(n)) => format!("  (95% CI [{lo:.4}, {hi:.4}], n={n})"),
                _ => String::new(),
            };
            let wilson = match (report.risk_wilson_upper, report.wilson_confidence_level) {
                (Some(w), Some(level)) => {
                    format!("  (Wilson {:.0}% upper bound: {w:.4})", level * 100.0)
                }
                _ => String::new(),
            };
            eprintln!(
                "  observed risk:      {:.4}  ({}/{} errors)  (target was {:.4}){risk_ci}{wilson}",
                risk, report.errors, report.accepted, report.target_error_rate
            );
            if let Some(true) = report.target_risk_exceeded {
                eprintln!("  WARNING: observed risk exceeds target error rate.");
            }
        }
        None => {
            eprintln!("  observed risk:      n/a  (0 accepted)");
            if let Some(reason) = &report.abstain_reason {
                eprintln!("  ABSTAIN-ALL: {reason}");
            }
        }
    }
}
