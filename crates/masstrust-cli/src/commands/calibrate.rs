use std::path::PathBuf;

use clap::Args;
use masstrust_core::{
    calibration::{calibrate_binomial, calibrate_empirical},
    io, metrics, CalibrationMethod, PolicyFile,
};

use super::parse_scoring_method;

#[derive(Args)]
pub struct CalibrateArgs {
    /// Input labeled candidates CSV
    pub input: PathBuf,
    /// Scoring method
    #[arg(long)]
    pub score: String,
    /// Target error rate (e.g. 0.05)
    #[arg(long)]
    pub error_rate: f64,
    /// Calibration method: empirical or binomial
    #[arg(long)]
    pub method: String,
    /// Confidence level for binomial (e.g. 0.95)
    #[arg(long)]
    pub confidence_level: Option<f64>,
    /// Output policy JSON path
    #[arg(long)]
    pub out: PathBuf,
    /// Output SVG risk-coverage plot (requires --features plot)
    #[arg(long)]
    pub plot: Option<PathBuf>,
}

pub fn run(args: CalibrateArgs) -> anyhow::Result<()> {
    let scoring_method = parse_scoring_method(&args.score)?;
    let calibration_method = match args.method.as_str() {
        "empirical" => CalibrationMethod::Empirical,
        "binomial" => CalibrationMethod::Binomial,
        other => anyhow::bail!(
            "Unknown calibration method: '{}'. Valid: empirical, binomial",
            other
        ),
    };

    let candidates = io::read_candidates(&args.input)?;
    let rankings = io::group_by_query(candidates);
    let curve = metrics::compute_curve(&rankings, scoring_method);

    let threshold_opt = match calibration_method {
        CalibrationMethod::Empirical => calibrate_empirical(&curve, args.error_rate),
        CalibrationMethod::Binomial => {
            let level = args.confidence_level.ok_or_else(|| {
                anyhow::anyhow!("--confidence-level required for binomial method")
            })?;
            calibrate_binomial(&curve, args.error_rate, level)?
        }
    };

    let threshold = match threshold_opt {
        Some(t) => t,
        None => {
            eprintln!(
                "WARNING: No threshold satisfies error rate {:.4}. Accepting nothing (threshold = +inf).",
                args.error_rate
            );
            f64::MAX
        }
    };

    let policy = PolicyFile {
        version: "0.1.0".into(),
        scoring_method,
        threshold,
        target_error_rate: args.error_rate,
        calibration_method,
        confidence_level: args.confidence_level,
        created_by: "masstrust".into(),
    };

    io::write_json(&policy, &args.out)?;
    print_calibration_report(&curve, &policy, threshold_opt);

    #[cfg(feature = "plot")]
    if let Some(ref plot_path) = args.plot {
        crate::plot::svg::render(&curve, plot_path, Some(args.error_rate))?;
        eprintln!("  SVG:      {}", plot_path.display());
    }

    #[cfg(not(feature = "plot"))]
    if args.plot.is_some() {
        eprintln!("WARNING: --plot ignored; recompile with --features plot to enable SVG output");
    }

    Ok(())
}

fn print_calibration_report(
    curve: &[masstrust_core::RiskCoverageRow],
    policy: &PolicyFile,
    threshold_opt: Option<f64>,
) {
    let cal_method = format!("{:?}", policy.calibration_method).to_lowercase();
    let score_method = format!("{:?}", policy.scoring_method);
    eprintln!("Calibration result ({score_method}, {cal_method}):");
    eprintln!("  target error rate: {:.4}", policy.target_error_rate);

    if threshold_opt.is_none() {
        eprintln!("  threshold:         +inf  (no valid threshold found)");
        eprintln!("  coverage:          0.0000  (0 queries accepted)");
        return;
    }

    eprintln!("  threshold:         {:.6}", policy.threshold);

    // Find the row matching the calibrated threshold
    if let Some(row) = curve.iter().find(|r| r.threshold == policy.threshold) {
        let pct = row.coverage * 100.0;
        eprintln!(
            "  coverage:          {:.4}  ({}/{} queries accepted, {pct:.1}%)",
            row.coverage, row.accepted, row.total
        );
        if let Some(risk) = row.risk {
            eprintln!(
                "  observed risk:     {:.4}  ({}/{} errors)",
                risk, row.errors, row.accepted
            );
        }
    }

    let aurc = metrics::compute_aurc(curve);
    let eaurc = metrics::compute_eaurc(curve);
    if aurc.is_finite() {
        eprintln!("  AURC:              {aurc:.6}");
    }
    if eaurc.is_finite() {
        eprintln!("  E-AURC:            {eaurc:.6}");
    }

    eprintln!("  policy written to: see --out");
}
