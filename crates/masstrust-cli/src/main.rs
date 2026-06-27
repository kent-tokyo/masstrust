use clap::{Parser, Subcommand};

mod commands;
mod plot;
use commands::{apply, batch, calibrate, curve};

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
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Curve(args) => curve::run(args),
        Commands::Calibrate(args) => calibrate::run(args),
        Commands::Apply(args) => apply::run(args),
        Commands::Batch(args) => batch::run(args),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
