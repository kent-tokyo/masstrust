use std::collections::HashSet;
use std::path::PathBuf;

use clap::Args;
use masstrust_core::io;

#[derive(Args)]
pub struct ValidateSplitArgs {
    /// Calibration CSV
    #[arg(long)]
    pub calibration: PathBuf,
    /// Test / application CSV
    #[arg(long)]
    pub test: PathBuf,
}

pub fn run(args: ValidateSplitArgs) -> anyhow::Result<()> {
    let cal = io::read_candidates(&args.calibration)?;
    let tst = io::read_candidates(&args.test)?;

    let n_cal = io::group_by_query(cal.clone()).len();
    let n_tst_rankings = io::group_by_query(tst.clone());
    let n_tst = n_tst_rankings.len();

    println!("Leakage check: calibration={n_cal} queries, test={n_tst} queries");

    let cal_qids: HashSet<&str> = cal.iter().map(|c| c.query_id.as_str()).collect();
    let tst_qids: Vec<&str> = n_tst_rankings.iter().map(|r| r.query_id.as_str()).collect();
    let qid_overlap = tst_qids.iter().filter(|q| cal_qids.contains(*q)).count();
    let qid_pct = if n_tst > 0 {
        qid_overlap as f64 / n_tst as f64 * 100.0
    } else {
        0.0
    };
    println!("  query_id overlap:   {qid_overlap} / {n_tst} test queries ({qid_pct:.1}%)");

    // ponytail: any(is_some()) as proxy for column present; all-None is indistinguishable from absent
    let mut overlap_found = qid_overlap > 0;

    if cal.iter().any(|c| c.inchikey.is_some()) && tst.iter().any(|c| c.inchikey.is_some()) {
        let cal_ik: HashSet<&str> = cal.iter().filter_map(|c| c.inchikey.as_deref()).collect();
        let tst_ik: HashSet<&str> = tst.iter().filter_map(|c| c.inchikey.as_deref()).collect();
        let n = cal_ik.intersection(&tst_ik).count();
        println!("  inchikey overlap:   {n} unique inchikeys shared");
        if n > 0 {
            overlap_found = true;
        }
    }

    if cal.iter().any(|c| c.formula.is_some()) && tst.iter().any(|c| c.formula.is_some()) {
        let cal_fm: HashSet<&str> = cal.iter().filter_map(|c| c.formula.as_deref()).collect();
        let tst_fm: HashSet<&str> = tst.iter().filter_map(|c| c.formula.as_deref()).collect();
        let n = cal_fm.intersection(&tst_fm).count();
        println!("  formula overlap:    {n} unique formulas shared");
        if n > 0 {
            overlap_found = true;
        }
    }

    if overlap_found {
        println!("WARNING: Data leakage detected. Results may be optimistic.");
        use std::io::Write;
        std::io::stdout().flush().ok();
        // ponytail: direct exit, avoids double messaging via Err path
        std::process::exit(1);
    } else {
        println!("No overlap detected.");
    }
    Ok(())
}
