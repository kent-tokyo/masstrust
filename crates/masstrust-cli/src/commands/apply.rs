use std::path::PathBuf;

use clap::Args;
use masstrust_core::{io, policy};

#[derive(Args)]
pub struct ApplyArgs {
    /// Input candidates CSV (may be unlabeled)
    pub input: PathBuf,
    /// Trust policy JSON
    #[arg(long)]
    pub policy: PathBuf,
    /// Output trusted annotations CSV
    #[arg(long)]
    pub out: PathBuf,
    /// Output abstained queries CSV
    #[arg(long)]
    pub abstained: PathBuf,
}

pub fn run(args: ApplyArgs) -> anyhow::Result<()> {
    let p = policy::load_policy(&args.policy)?;
    let candidates = io::read_candidates(&args.input)?;
    let rankings = io::group_by_query(candidates);
    let decisions = policy::apply_policy(&rankings, &p);

    let (trusted, abstained): (Vec<_>, Vec<_>) = decisions.into_iter().partition(|d| d.accepted);

    io::write_csv(&trusted, &args.out)?;
    io::write_csv(&abstained, &args.abstained)?;
    eprintln!(
        "Accepted: {}  Abstained: {}  (wrote {} and {})",
        trusted.len(),
        abstained.len(),
        args.out.display(),
        args.abstained.display()
    );
    Ok(())
}
