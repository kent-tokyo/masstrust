use std::collections::HashSet;
use std::path::PathBuf;

use clap::Args;
use masstrust_core::io;
use serde::Serialize;

#[derive(Args)]
pub struct ValidateSplitArgs {
    /// Calibration CSV
    #[arg(long)]
    pub calibration: PathBuf,
    /// Test / application CSV
    #[arg(long)]
    pub test: PathBuf,
    /// Output validation report JSON path
    #[arg(long)]
    pub out: Option<PathBuf>,
}

#[derive(Serialize)]
struct SplitValidationReport {
    n_calibration_queries: usize,
    n_test_queries: usize,
    /// Same spectrum (query_id) appearing in both splits — unambiguous leakage, hard failure.
    query_id_overlap: usize,
    query_id_overlap_pct: f64,
    /// Same candidate structure recurring in both splits' candidate pools — expected and
    /// benign on its own (independently-sampled queries commonly share pool molecules),
    /// stats only. Uses `inchikey` when present, else falls back to `candidate_id`.
    candidate_pool_overlap: usize,
    /// Same molecular formula recurring in both splits — very common for unrelated
    /// molecules, stats only.
    formula_overlap: usize,
    /// A val query's correct answer is also a test query's correct answer (by full
    /// InChIKey). Stronger leakage signal than pool overlap, but MassSpecGym's split
    /// guarantee for this isn't verified here — reported, not hard-failed.
    target_inchikey_overlap: usize,
    /// Same as above, ignoring stereochemistry/protonation (first InChIKey block).
    target_inchikey_skeleton_overlap: usize,
    hard_failure: bool,
}

fn inchikey_skeleton(key: &str) -> &str {
    key.split('-').next().unwrap_or(key)
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
    let query_id_overlap = tst_qids.iter().filter(|q| cal_qids.contains(*q)).count();
    let query_id_overlap_pct = if n_tst > 0 {
        query_id_overlap as f64 / n_tst as f64 * 100.0
    } else {
        0.0
    };
    println!(
        "  query_id overlap:   {query_id_overlap} / {n_tst} test queries ({query_id_overlap_pct:.1}%)"
    );

    // Candidate-pool overlap: always computable (candidate_id is a required column).
    // `inchikey` is used when present since it's the more precise chemical identity;
    // otherwise candidate_id itself (which is frequently an InChIKey in practice, e.g.
    // the MassSpecGym harness) stands in.
    let cal_pool: HashSet<&str> = cal
        .iter()
        .map(|c| c.inchikey.as_deref().unwrap_or(c.candidate_id.as_str()))
        .collect();
    let tst_pool: HashSet<&str> = tst
        .iter()
        .map(|c| c.inchikey.as_deref().unwrap_or(c.candidate_id.as_str()))
        .collect();
    let candidate_pool_overlap = cal_pool.intersection(&tst_pool).count();
    println!(
        "  candidate pool overlap: {candidate_pool_overlap} unique structures shared (stats only, not leakage by itself)"
    );

    let mut formula_overlap = 0usize;
    if cal.iter().any(|c| c.formula.is_some()) && tst.iter().any(|c| c.formula.is_some()) {
        let cal_fm: HashSet<&str> = cal.iter().filter_map(|c| c.formula.as_deref()).collect();
        let tst_fm: HashSet<&str> = tst.iter().filter_map(|c| c.formula.as_deref()).collect();
        formula_overlap = cal_fm.intersection(&tst_fm).count();
        println!(
            "  formula overlap:    {formula_overlap} unique formulas shared (stats only, not leakage by itself)"
        );
    }

    let mut target_inchikey_overlap = 0usize;
    let mut target_inchikey_skeleton_overlap = 0usize;
    if cal.iter().any(|c| c.target_inchikey.is_some())
        && tst.iter().any(|c| c.target_inchikey.is_some())
    {
        let cal_tgt: HashSet<&str> = cal
            .iter()
            .filter_map(|c| c.target_inchikey.as_deref())
            .collect();
        let tst_tgt: HashSet<&str> = tst
            .iter()
            .filter_map(|c| c.target_inchikey.as_deref())
            .collect();
        target_inchikey_overlap = cal_tgt.intersection(&tst_tgt).count();

        let cal_skel: HashSet<&str> = cal_tgt.iter().map(|k| inchikey_skeleton(k)).collect();
        let tst_skel: HashSet<&str> = tst_tgt.iter().map(|k| inchikey_skeleton(k)).collect();
        target_inchikey_skeleton_overlap = cal_skel.intersection(&tst_skel).count();

        if target_inchikey_overlap > 0 || target_inchikey_skeleton_overlap > 0 {
            println!(
                "  WARNING: ANSWER LEAKAGE — {target_inchikey_overlap} exact target molecules \
                 (and {target_inchikey_skeleton_overlap} by 2D skeleton) are the correct answer \
                 in both splits. Not hard-failed: MassSpecGym's split guarantee for target-molecule \
                 disjointness isn't verified without the real dataset. Review before trusting results."
            );
        }
    }

    // Only query_id overlap is unambiguous leakage (the same spectrum evaluated in both
    // splits). Pool/formula/target overlap are reported above but never hard-fail.
    let hard_failure = query_id_overlap > 0;

    let report = SplitValidationReport {
        n_calibration_queries: n_cal,
        n_test_queries: n_tst,
        query_id_overlap,
        query_id_overlap_pct,
        candidate_pool_overlap,
        formula_overlap,
        target_inchikey_overlap,
        target_inchikey_skeleton_overlap,
        hard_failure,
    };
    if let Some(out_path) = &args.out {
        io::write_json(&report, out_path)?;
        eprintln!("  report written to: {}", out_path.display());
    }

    if hard_failure {
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
