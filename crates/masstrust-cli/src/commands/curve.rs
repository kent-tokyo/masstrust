use std::path::PathBuf;

use clap::Args;
#[cfg(feature = "plot")]
use masstrust_core::scoring;
use masstrust_core::{io, metrics};

use super::parse_scoring_method;

#[derive(Args)]
pub struct CurveArgs {
    /// Input labeled candidates CSV
    pub input: PathBuf,
    /// Scoring method: max-prob, score-gap, margin, entropy
    #[arg(long)]
    pub score: String,
    /// Output risk-coverage CSV path
    #[arg(long)]
    pub out: PathBuf,
    /// Print per-row table to stdout
    #[arg(long)]
    pub verbose: bool,
    /// Output SVG risk-coverage plot (requires --features plot)
    #[arg(long)]
    pub plot: Option<PathBuf>,
    /// Output SVG confidence histogram (requires --features plot)
    #[arg(long)]
    pub histogram: Option<PathBuf>,
}

pub fn run(args: CurveArgs) -> anyhow::Result<()> {
    let method = parse_scoring_method(&args.score)?;
    let candidates = io::read_candidates(&args.input)?;
    let rankings = io::group_by_query(candidates);
    let curve = metrics::compute_curve(&rankings, method);

    io::write_csv(&curve, &args.out)?;

    let aurc = metrics::compute_aurc(&curve);
    let eaurc = metrics::compute_eaurc(&curve);
    eprintln!("Wrote {} rows to {}", curve.len(), args.out.display());
    if aurc.is_finite() {
        eprintln!("  AURC:   {aurc:.6}");
    }
    if eaurc.is_finite() {
        eprintln!("  E-AURC: {eaurc:.6}");
    }

    if args.verbose {
        print_curve_table(&curve);
    }

    #[cfg(feature = "plot")]
    {
        if let Some(ref plot_path) = args.plot {
            crate::plot::svg::render(&curve, plot_path, None)?;
            eprintln!("  SVG:    {}", plot_path.display());
        }
        if let Some(ref hist_path) = args.histogram {
            let confidences: Vec<Option<f64>> = rankings
                .iter()
                .map(|r| scoring::compute_confidence(r, method))
                .collect();
            crate::plot::svg::histogram(&confidences, hist_path)?;
            eprintln!("  Hist:   {}", hist_path.display());
        }
    }

    #[cfg(not(feature = "plot"))]
    {
        if args.plot.is_some() {
            eprintln!(
                "WARNING: --plot ignored; recompile with --features plot to enable SVG output"
            );
        }
        if args.histogram.is_some() {
            eprintln!(
                "WARNING: --histogram ignored; recompile with --features plot to enable SVG output"
            );
        }
    }

    Ok(())
}

fn print_curve_table(curve: &[masstrust_core::RiskCoverageRow]) {
    println!(
        "{:<12} {:<10} {:<10} {:<8} {:<8} risk",
        "threshold", "coverage", "accepted", "total", "errors"
    );
    println!("{}", "-".repeat(58));
    for row in curve {
        let risk_str = row.risk.map_or("—".to_string(), |r| format!("{r:.4}"));
        println!(
            "{:<12.6} {:<10.4} {:<10} {:<8} {:<8} {}",
            row.threshold, row.coverage, row.accepted, row.total, row.errors, risk_str
        );
    }
}
