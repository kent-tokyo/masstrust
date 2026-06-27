use std::path::{Path, PathBuf};

use clap::Args;
use masstrust_core::{io, policy};

#[derive(Args)]
pub struct BatchArgs {
    /// Input candidate CSV (or Parquet with --features parquet) files
    pub inputs: Vec<PathBuf>,
    /// Trust policy JSON
    #[arg(long)]
    pub policy: PathBuf,
    /// Output directory (created if absent)
    #[arg(long)]
    pub out_dir: PathBuf,
}

pub fn run(args: BatchArgs) -> anyhow::Result<()> {
    if args.inputs.is_empty() {
        anyhow::bail!("No input files specified");
    }

    std::fs::create_dir_all(&args.out_dir)?;
    let p = policy::load_policy(&args.policy)?;

    let mut total_accepted = 0usize;
    let mut total_abstained = 0usize;

    for input in &args.inputs {
        let (accepted, abstained) = process_file(input, &p, &args.out_dir)?;
        eprintln!(
            "  {} → accepted: {}  abstained: {}",
            input.display(),
            accepted,
            abstained
        );
        total_accepted += accepted;
        total_abstained += abstained;
    }

    eprintln!(
        "Batch done: {} files  total accepted: {}  total abstained: {}",
        args.inputs.len(),
        total_accepted,
        total_abstained
    );
    Ok(())
}

fn process_file(
    input: &Path,
    p: &masstrust_core::PolicyFile,
    out_dir: &Path,
) -> anyhow::Result<(usize, usize)> {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let candidates = io::read_candidates(input)?;
    let rankings = io::group_by_query(candidates);
    let decisions = policy::apply_policy(&rankings, p);

    let (trusted, abstained): (Vec<_>, Vec<_>) = decisions.into_iter().partition(|d| d.accepted);

    io::write_csv(&trusted, &out_dir.join(format!("{stem}_trusted.csv")))?;
    io::write_csv(&abstained, &out_dir.join(format!("{stem}_abstained.csv")))?;

    Ok((trusted.len(), abstained.len()))
}
