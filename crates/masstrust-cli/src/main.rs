use clap::{Parser, Subcommand};

mod commands;
mod plot;
#[cfg(feature = "risksieve")]
use commands::certify_batch;
use commands::{apply, batch, calibrate, compare, curve, drift, evaluate, validate_split};

#[derive(Parser)]
#[command(
    name = "masstrust",
    about = "Calibrated trust and abstention for MS/MS molecular annotations",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a risk-coverage curve from labeled candidates
    Curve(curve::CurveArgs),
    /// Calibrate a trust threshold and export a policy
    Calibrate(calibrate::CalibrateArgs),
    /// Apply a saved policy to new candidate rankings
    Apply(apply::ApplyArgs),
    /// Apply a policy to multiple input files (batch mode)
    Batch(batch::BatchArgs),
    /// Compare multiple scoring methods on one labeled dataset
    Compare(compare::CompareArgs),
    /// Detect confidence distribution shift between calibration and new data
    Drift(drift::DriftArgs),
    /// Evaluate a fixed policy threshold on separate, labeled held-out data
    Evaluate(evaluate::EvaluateArgs),
    /// Check for data leakage between calibration and test sets
    ValidateSplit(validate_split::ValidateSplitArgs),
    /// Theorem-backed batch selective-deployment certification (risksieve SCoRE-SDR).
    /// Not a reusable threshold policy — see `docs/risksieve-integration.md`.
    #[cfg(feature = "risksieve")]
    CertifyBatch(certify_batch::CertifyBatchArgs),
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Curve(args) => curve::run(args),
        Commands::Calibrate(args) => calibrate::run(args),
        Commands::Apply(args) => apply::run(args),
        Commands::Batch(args) => batch::run(args),
        Commands::Compare(args) => compare::run(args),
        Commands::Drift(args) => drift::run(args),
        Commands::Evaluate(args) => evaluate::run(args),
        Commands::ValidateSplit(args) => validate_split::run(args),
        #[cfg(feature = "risksieve")]
        Commands::CertifyBatch(args) => certify_batch::run(args),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
