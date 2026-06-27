use std::collections::HashMap;
use std::path::PathBuf;

use clap::Args;
use masstrust_core::{
    calibration::{calibrate_binomial, calibrate_crc, calibrate_empirical, calibrate_grouped},
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
    /// Calibration method: empirical, binomial, or crc
    #[arg(long)]
    pub method: String,
    /// Confidence level for binomial (e.g. 0.95)
    #[arg(long)]
    pub confidence_level: Option<f64>,
    /// CSV column to use for grouped calibration (e.g. adduct, instrument)
    #[arg(long)]
    pub group_col: Option<String>,
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
        "crc" => CalibrationMethod::Crc,
        other => anyhow::bail!(
            "Unknown calibration method: '{}'. Valid: empirical, binomial, crc",
            other
        ),
    };

    let mut candidates = io::read_candidates(&args.input)?;

    // Attach group column if requested.
    if let Some(group_col) = &args.group_col {
        let groups = io::read_group_column(&args.input, group_col)?;
        for (c, g) in candidates.iter_mut().zip(groups) {
            c.group = g;
        }
    }

    let rankings = io::group_by_query(candidates);

    // Global curve + threshold (always computed as fallback).
    let curve = metrics::compute_curve(&rankings, scoring_method);
    let global_threshold_opt = match calibration_method {
        CalibrationMethod::Empirical => calibrate_empirical(&curve, args.error_rate),
        CalibrationMethod::Crc => calibrate_crc(&curve, args.error_rate),
        CalibrationMethod::Binomial => {
            let level = args.confidence_level.ok_or_else(|| {
                anyhow::anyhow!("--confidence-level required for binomial method")
            })?;
            calibrate_binomial(&curve, args.error_rate, level)?
        }
    };

    let global_threshold = match global_threshold_opt {
        Some(t) => t,
        None => {
            eprintln!(
                "WARNING: No threshold satisfies error rate {:.4} on the full dataset. \
                 Accepting nothing (threshold = +inf).",
                args.error_rate
            );
            f64::MAX
        }
    };

    // Per-group calibration (if --group-col specified).
    let mut group_thresholds: Option<HashMap<String, f64>> = None;
    if args.group_col.is_some() {
        let result = calibrate_grouped(
            &rankings,
            scoring_method,
            args.error_rate,
            calibration_method,
            args.confidence_level,
        )?;

        for g in &result.fallback_groups {
            eprintln!(
                "WARNING: group '{g}' has no valid threshold at {:.4}; using global fallback ({global_threshold:.6}).",
                args.error_rate
            );
        }

        group_thresholds = Some(result.thresholds);
    }

    let policy = PolicyFile {
        version: "0.1.0".into(),
        scoring_method,
        threshold: global_threshold,
        target_error_rate: args.error_rate,
        calibration_method,
        confidence_level: args.confidence_level,
        created_by: "masstrust".into(),
        group_col: args.group_col.clone(),
        group_thresholds,
    };

    io::write_json(&policy, &args.out)?;
    print_calibration_report(&curve, &policy, global_threshold_opt);

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
    global_threshold_opt: Option<f64>,
) {
    let cal_method = format!("{:?}", policy.calibration_method).to_lowercase();
    let score_method = format!("{:?}", policy.scoring_method);
    let group_label = policy
        .group_col
        .as_deref()
        .map_or(String::new(), |g| format!(", grouped by {g}"));

    eprintln!("Calibration result ({score_method}, {cal_method}{group_label}):");
    eprintln!("  target error rate: {:.4}", policy.target_error_rate);

    // Per-group thresholds
    if let Some(gt) = &policy.group_thresholds {
        let mut groups: Vec<_> = gt.iter().collect();
        groups.sort_by_key(|(k, _)| k.as_str());
        for (group, &threshold) in &groups {
            eprintln!("  [{group}] threshold: {threshold:.6}");
        }
    }

    // Global threshold
    if global_threshold_opt.is_none() {
        eprintln!("  global threshold:  +inf  (no valid threshold found)");
        eprintln!("  coverage:          0.0000  (0 queries accepted)");
        return;
    }

    eprintln!("  global threshold:  {:.6}", policy.threshold);
    if policy.calibration_method == masstrust_core::CalibrationMethod::Crc {
        if let Some(row) = curve.last() {
            let correction = 1.0 / (row.total as f64 + 1.0);
            eprintln!(
                "  CRC correction:    {correction:.6}  (1/(n+1), n={total})",
                total = row.total
            );
        }
    }

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
