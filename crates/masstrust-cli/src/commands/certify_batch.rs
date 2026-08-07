use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use anyhow::bail;
use clap::Args;
use masstrust_core::risksieve_backend::{self, BatchCertification, Construction};
use masstrust_core::{ScoringMethod, io};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::parse_scoring_method;

#[derive(Args)]
pub struct CertifyBatchArgs {
    /// Labeled calibration candidates CSV
    #[arg(long)]
    pub calibration: PathBuf,
    /// Test candidates CSV (labels optional — only used for realized risk if present)
    #[arg(long)]
    pub test: PathBuf,
    /// Scoring method
    #[arg(long)]
    pub score: String,
    /// Target selective deployment risk (must be in the open interval (0, 1))
    #[arg(long)]
    pub alpha: f64,
    /// Risk-adjusted e-value calibration parameter (must be in (0, 1); need not satisfy
    /// gamma <= alpha — Theorem 4.2 gives validity for any gamma in (0,1); it only affects
    /// selection power, per Remark 4.5)
    #[arg(long)]
    pub gamma: f64,
    /// e-value construction: "coupled" (paper-exact, default) or "independent"
    #[arg(long, default_value = "coupled")]
    pub construction: String,
    /// Output accepted (selected) queries CSV
    #[arg(long)]
    pub accepted: PathBuf,
    /// Output abstained queries CSV
    #[arg(long)]
    pub abstained: PathBuf,
    /// Output certificate JSON
    #[arg(long)]
    pub certificate: PathBuf,
    /// Output Markdown report
    #[arg(long)]
    pub report: PathBuf,
}

fn parse_construction(s: &str) -> anyhow::Result<Construction> {
    match s {
        "coupled" => Ok(Construction::Coupled),
        "independent" => Ok(Construction::Independent),
        other => bail!("Unknown construction: '{other}'. Valid: coupled, independent"),
    }
}

fn construction_str(c: Construction) -> &'static str {
    match c {
        Construction::Coupled => "coupled",
        Construction::Independent => "independent",
    }
}

fn sha256_file(path: &PathBuf) -> anyhow::Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Serialize)]
struct AcceptedRow<'a> {
    query_id: &'a str,
    candidate_id: &'a str,
    rank: usize,
    raw_score: f64,
    confidence: f64,
    selected: bool,
    selection_index: usize,
    scoring_method: String,
    backend: &'static str,
    construction: &'static str,
}

#[derive(Serialize)]
struct AbstainedRow<'a> {
    query_id: &'a str,
    reason: &'a str,
    scoring_method: String,
    backend: &'static str,
    construction: &'static str,
}

#[derive(Serialize)]
struct InputFileHashes {
    calibration_path: String,
    calibration_sha256: String,
    test_path: String,
    test_sha256: String,
}

/// Certificate output schema. Deliberately separate from `PolicyFile` (see
/// `docs/risksieve-integration.md`) — never merged into it, never versioned alongside it.
#[derive(Serialize)]
struct CertificateEnvelope<'a> {
    schema_version: &'static str,
    masstrust_version: &'static str,
    risksieve_dependency: &'static str,
    guarantee_kind: String,
    target_risk: f64,
    certified_upper_bound: f64,
    scoring_method: String,
    score_orientation: &'a str,
    query_ordering_policy: &'a str,
    alpha: f64,
    gamma: f64,
    construction: &'static str,
    calibration_total: usize,
    calibration_scoreable: usize,
    test_total: usize,
    test_scoreable: usize,
    selected_count: usize,
    abstained_count: usize,
    selected_query_ids: Vec<&'a str>,
    selected_indices: &'a [usize],
    input_files: InputFileHashes,
    command: String,
    created_by: &'static str,
    certificate: risksieve::RiskCertificate<Vec<usize>>,
    realized_selective_risk: Option<f64>,
}

pub fn run(args: CertifyBatchArgs) -> anyhow::Result<()> {
    let scoring_method = parse_scoring_method(&args.score)?;
    let construction = parse_construction(&args.construction)?;

    let calibration_candidates = io::read_candidates(&args.calibration)?;
    let calibration_rankings = io::group_by_query(calibration_candidates);
    let test_candidates = io::read_candidates(&args.test)?;
    let test_rankings = io::group_by_query(test_candidates);

    let certification = risksieve_backend::certify_batch(
        &calibration_rankings,
        &test_rankings,
        scoring_method,
        args.alpha,
        args.gamma,
        construction,
    )?;

    // Realized risk only if the test data actually carries labels — a post-hoc descriptive
    // statistic, never presented as validating the certificate. See docs/risksieve-integration.md.
    let has_test_labels = test_rankings.iter().any(|r| {
        r.candidates
            .iter()
            .min_by_key(|c| c.rank)
            .is_some_and(|c| c.is_correct.is_some())
    });
    let realized_risk = if has_test_labels && !certification.certificate.parameter.is_empty() {
        let losses = risksieve_backend::resolve_realized_losses(&certification, &test_rankings)?;
        Some(risksieve::selective::sdr::realized_selective_risk(&losses))
    } else if has_test_labels {
        // Empty selection: risksieve's own convention is 0.0, not NaN — reuse it rather than
        // special-casing the empty-slice call ourselves.
        Some(risksieve::selective::sdr::realized_selective_risk(&[]))
    } else {
        None
    };

    write_outputs(
        &args,
        &certification,
        &test_rankings,
        scoring_method,
        construction,
        realized_risk,
    )?;
    print_summary(&certification, scoring_method, construction, realized_risk);

    Ok(())
}

fn write_outputs(
    args: &CertifyBatchArgs,
    certification: &BatchCertification,
    test_rankings: &[masstrust_core::QueryRanking],
    scoring_method: ScoringMethod,
    construction: Construction,
    realized_risk: Option<f64>,
) -> anyhow::Result<()> {
    let selected: HashSet<usize> = certification
        .certificate
        .parameter
        .iter()
        .copied()
        .collect();
    let backend = "risksieve";
    let construction_name = construction_str(construction);
    let method_name = format!("{scoring_method:?}");

    let test_by_id: std::collections::HashMap<&str, &masstrust_core::QueryRanking> = test_rankings
        .iter()
        .map(|r| (r.query_id.as_str(), r))
        .collect();

    let mut accepted_rows = Vec::new();
    let mut abstained_rows = Vec::new();
    for (idx, sq) in certification.scoreable_test_queries.iter().enumerate() {
        let ranking = test_by_id
            .get(sq.query_id.as_str())
            .expect("scoreable_test_queries is derived from test_rankings");
        let top1 = ranking
            .candidates
            .iter()
            .min_by_key(|c| c.rank)
            .expect("QueryRanking is never empty");
        if selected.contains(&idx) {
            accepted_rows.push(AcceptedRow {
                query_id: &sq.query_id,
                candidate_id: &top1.candidate_id,
                rank: top1.rank,
                raw_score: top1.score,
                confidence: sq.confidence,
                selected: true,
                selection_index: idx,
                scoring_method: method_name.clone(),
                backend,
                construction: construction_name,
            });
        } else {
            abstained_rows.push(AbstainedRow {
                query_id: &sq.query_id,
                reason: "not_selected_by_certificate",
                scoring_method: method_name.clone(),
                backend,
                construction: construction_name,
            });
        }
    }
    for uq in &certification.unscoreable_test_queries {
        abstained_rows.push(AbstainedRow {
            query_id: &uq.query_id,
            reason: uq.reason,
            scoring_method: method_name.clone(),
            backend,
            construction: construction_name,
        });
    }

    io::write_csv(&accepted_rows, &args.accepted)?;
    io::write_csv(&abstained_rows, &args.abstained)?;

    let selected_query_ids = certification.selected_query_ids();
    let envelope = CertificateEnvelope {
        schema_version: "1.0",
        masstrust_version: env!("CARGO_PKG_VERSION"),
        risksieve_dependency: "risksieve 0.2.0 (crates.io)",
        guarantee_kind: format!("{:?}", certification.certificate.guarantee),
        target_risk: certification.certificate.target_risk,
        certified_upper_bound: certification.certificate.certified_upper_bound,
        scoring_method: method_name.clone(),
        score_orientation: certification.score_orientation_note,
        query_ordering_policy: certification.query_ordering_policy,
        alpha: args.alpha,
        gamma: args.gamma,
        construction: construction_name,
        calibration_total: certification.calibration_counts.total,
        calibration_scoreable: certification.calibration_counts.scoreable,
        test_total: certification.test_counts.total,
        test_scoreable: certification.test_counts.scoreable,
        selected_count: certification.certificate.parameter.len(),
        abstained_count: certification.test_counts.total
            - certification.certificate.parameter.len(),
        selected_query_ids,
        selected_indices: &certification.certificate.parameter,
        input_files: InputFileHashes {
            calibration_path: args.calibration.display().to_string(),
            calibration_sha256: sha256_file(&args.calibration)?,
            test_path: args.test.display().to_string(),
            test_sha256: sha256_file(&args.test)?,
        },
        command: std::env::args().collect::<Vec<_>>().join(" "),
        created_by: "masstrust certify-batch",
        certificate: certification.certificate.clone(),
        realized_selective_risk: realized_risk,
    };
    io::write_json(&envelope, &args.certificate)?;

    let report = render_report(certification, scoring_method, construction, realized_risk);
    fs::write(&args.report, report)?;

    Ok(())
}

fn render_report(
    certification: &BatchCertification,
    scoring_method: ScoringMethod,
    construction: Construction,
    realized_risk: Option<f64>,
) -> String {
    let cert = &certification.certificate;
    let mut s = String::new();
    s.push_str("# masstrust certify-batch report\n\n");
    s.push_str(
        "**This is a batch selective-deployment certificate, not a reusable threshold policy.** ",
    );
    s.push_str("Selection was computed jointly with this specific test batch and does not transfer to future batches. ");
    s.push_str("See `docs/risksieve-integration.md`.\n\n");

    s.push_str("## Theorem-backed certificate\n\n");
    s.push_str("Expected selective deployment risk under the stated assumptions — a property of the expectation over the joint draw of calibration and the entire test batch, **not** a guarantee about this one realized batch.\n\n");
    s.push_str("- backend: risksieve\n");
    s.push_str(&format!(
        "- construction: {}\n",
        construction_str(construction)
    ));
    s.push_str(&format!("- scoring method: {scoring_method:?}\n"));
    s.push_str(&format!("- alpha (target risk): {}\n", cert.target_risk));
    s.push_str(&format!(
        "- certified upper bound: {}\n",
        cert.certified_upper_bound
    ));
    s.push_str(&format!(
        "- gamma: {}\n",
        cert.diagnostics
            .gamma
            .map(|g| g.to_string())
            .unwrap_or_default()
    ));
    s.push_str(&format!("- guarantee kind: {:?}\n", cert.guarantee));
    s.push_str(&format!(
        "- calibration queries: {} total, {} scoreable (used)\n",
        certification.calibration_counts.total, certification.calibration_counts.scoreable
    ));
    s.push_str(&format!(
        "- test queries: {} total, {} scoreable, {} unscoreable\n",
        certification.test_counts.total,
        certification.test_counts.scoreable,
        certification.test_counts.total - certification.test_counts.scoreable
    ));
    s.push_str(&format!(
        "- selected: {}  abstained: {}\n",
        cert.parameter.len(),
        certification.test_counts.total - cert.parameter.len()
    ));
    s.push_str(&format!(
        "- uninformative result: {}\n",
        cert.diagnostics
            .uninformative_result
            .map(|b| b.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    ));
    s.push_str(&format!(
        "- eBH tau_hat: {}\n",
        cert.diagnostics
            .ebh_tau_hat
            .map(|t| t.to_string())
            .unwrap_or_else(|| "none (empty selection)".to_string())
    ));
    s.push_str(&format!(
        "- score orientation: {}\n",
        certification.score_orientation_note
    ));
    s.push_str(&format!(
        "- query ordering policy: {}\n",
        certification.query_ordering_policy
    ));
    s.push_str(&format!("- assumptions: {:?}\n\n", cert.assumptions));

    if cert.parameter.is_empty() {
        s.push_str("Zero selections is a valid certificate, not an error — see risksieve's own `empty_batch_is_a_valid_empty_certificate` test and `docs/guarantees.md`.\n\n");
    }

    s.push_str("## Post-hoc descriptive result\n\n");
    match realized_risk {
        Some(risk) => {
            s.push_str(&format!(
                "Realized selective risk on this labeled batch: **{risk}**\n\n"
            ));
            s.push_str("This is a descriptive statistic computed from actual outcomes on this one batch, not a guarantee. ");
            s.push_str("It does not validate the certificate above, and a value at or below alpha here is not \"certificate verification succeeded\" — nor does a value above alpha, on its own, mean the theorem was violated (the guarantee is about an expectation over repeated draws, not this single batch).\n\n");
        }
        None => {
            s.push_str("Not computed: the test data did not carry `is_correct` labels.\n\n");
        }
    }

    s
}

fn print_summary(
    certification: &BatchCertification,
    scoring_method: ScoringMethod,
    construction: Construction,
    realized_risk: Option<f64>,
) {
    let cert = &certification.certificate;
    eprintln!(
        "certify-batch result (risksieve, {}):",
        construction_str(construction)
    );
    eprintln!("  scoring method:       {scoring_method:?}");
    eprintln!("  alpha:                {}", cert.target_risk);
    eprintln!(
        "  gamma:                {}",
        cert.diagnostics
            .gamma
            .map(|g| g.to_string())
            .unwrap_or_default()
    );
    eprintln!(
        "  calibration:          {} total, {} scoreable",
        certification.calibration_counts.total, certification.calibration_counts.scoreable
    );
    eprintln!(
        "  test:                 {} total, {} scoreable, {} unscoreable",
        certification.test_counts.total,
        certification.test_counts.scoreable,
        certification.test_counts.total - certification.test_counts.scoreable
    );
    eprintln!(
        "  selected:             {}  abstained: {}",
        cert.parameter.len(),
        certification.test_counts.total - cert.parameter.len()
    );
    eprintln!("  guarantee kind:       {:?}", cert.guarantee);
    eprintln!(
        "  uninformative result: {}",
        cert.diagnostics
            .uninformative_result
            .map(|b| b.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    eprintln!(
        "  eBH tau_hat:          {}",
        cert.diagnostics
            .ebh_tau_hat
            .map(|t| t.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    eprintln!(
        "  score orientation:    {}",
        certification.score_orientation_note
    );
    match realized_risk {
        Some(risk) => eprintln!("  realized risk (post-hoc, descriptive only): {risk}"),
        None => eprintln!("  realized risk:        not computed (no test labels)"),
    }
}
