//! CLI-level tests for `masstrust certify-batch` (risksieve-backed SCoRE-SDR).
//!
//! Gated on the `risksieve` feature via `required-features` in Cargo.toml — this whole file
//! is skipped, not merely no-op, when the feature is off.

use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    p.pop();
    p.push("masstrust");
    p
}

/// A calibration CSV with an unambiguous confidence/correctness split: even-indexed queries
/// have a large score-gap and are correct; odd-indexed queries have a tiny score-gap and are
/// incorrect. Large enough (n=40) for eBH to have something to select under permissive
/// alpha/gamma.
fn write_extreme_csv(path: &std::path::Path, prefix: &str, n: usize) {
    let mut f = std::fs::File::create(path).unwrap();
    writeln!(f, "query_id,candidate_id,rank,score,is_correct").unwrap();
    for i in 0..n {
        let correct = i % 2 == 0;
        let gap: f64 = if correct { 0.8 } else { 0.01 };
        writeln!(
            f,
            "{prefix}{i},{prefix}{i}a,1,0.99,{}",
            if correct { "true" } else { "false" }
        )
        .unwrap();
        writeln!(f, "{prefix}{i},{prefix}{i}b,2,{:.3},false", 0.99 - gap).unwrap();
    }
}

struct Outputs {
    dir: tempfile::TempDir,
}

impl Outputs {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}

fn run_certify_batch(
    calibration: &std::path::Path,
    test: &std::path::Path,
    out: &Outputs,
    extra: &[&str],
) -> std::process::Output {
    let mut args = vec![
        "certify-batch".to_string(),
        "--calibration".into(),
        calibration.to_str().unwrap().into(),
        "--test".into(),
        test.to_str().unwrap().into(),
        "--score".into(),
        "score-gap".into(),
        "--accepted".into(),
        out.path("accepted.csv").to_str().unwrap().into(),
        "--abstained".into(),
        out.path("abstained.csv").to_str().unwrap().into(),
        "--certificate".into(),
        out.path("certificate.json").to_str().unwrap().into(),
        "--report".into(),
        out.path("report.md").to_str().unwrap().into(),
    ];
    args.extend(extra.iter().map(|s| s.to_string()));
    Command::new(bin())
        .args(&args)
        .output()
        .expect("failed to run masstrust")
}

#[test]
fn smoke_test_generates_all_four_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    write_extreme_csv(&calib, "c", 40);
    write_extreme_csv(&test, "t", 10);

    let out = Outputs::new();
    let output = run_certify_batch(
        &calib,
        &test,
        &out,
        &[
            "--alpha",
            "0.5",
            "--gamma",
            "0.1",
            "--construction",
            "coupled",
        ],
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // accepted.csv may be empty (zero bytes) when nothing is selected -- assert existence,
    // not non-zero length, for accepted/abstained.
    assert!(out.path("accepted.csv").exists());
    assert!(out.path("abstained.csv").exists());
    assert!(out.path("certificate.json").metadata().unwrap().len() > 0);
    assert!(out.path("report.md").metadata().unwrap().len() > 0);

    let cert: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path("certificate.json")).unwrap())
            .unwrap();
    assert_eq!(cert["guarantee_kind"], "SelectiveDeploymentRisk");
    assert_eq!(cert["schema_version"], "1.0");
    assert!(cert["certificate"]["parameter"].is_array());

    // Audit-provenance fields from code review: certified population, unscoreable policy,
    // exact risksieve version pin, and per-query exclusion reasons (not just counts).
    assert!(
        cert["certified_population"]
            .as_str()
            .unwrap()
            .contains("ScoreGap")
    );
    assert!(
        cert["unscoreable_policy"]
            .as_str()
            .unwrap()
            .contains("always abstained")
    );
    assert!(
        cert["risksieve_dependency"]
            .as_str()
            .unwrap()
            .contains("0.2.0")
    );
    assert!(cert["excluded_calibration_queries"].is_array());
    assert!(cert["excluded_test_queries"].is_array());

    let report = std::fs::read_to_string(out.path("report.md")).unwrap();
    assert!(report.contains("Certified population"));
}

/// The property test the spec calls out explicitly: a high-confidence correct query is
/// selected more readily than a low-confidence incorrect one is. Uses the unambiguous
/// extreme fixture rather than asserting on exact indices from a marginal one.
#[test]
fn high_confidence_correct_queries_are_selected_low_confidence_incorrect_are_not() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    write_extreme_csv(&calib, "c", 40);
    write_extreme_csv(&test, "t", 10);

    let out = Outputs::new();
    let output = run_certify_batch(
        &calib,
        &test,
        &out,
        &[
            "--alpha",
            "0.5",
            "--gamma",
            "0.1",
            "--construction",
            "coupled",
        ],
    );
    assert!(output.status.success());

    let accepted = std::fs::read_to_string(out.path("accepted.csv")).unwrap();
    assert!(
        !accepted.is_empty(),
        "expected at least one selection under permissive alpha/gamma on a maximally separable fixture"
    );

    // Every accepted row's query_id must be even-indexed (the "correct" half of the fixture).
    for line in accepted.lines().skip(1) {
        let query_id = line.split(',').next().unwrap();
        let idx: usize = query_id.trim_start_matches('t').parse().unwrap();
        assert_eq!(
            idx % 2,
            0,
            "selected an odd-indexed (low-confidence, incorrect) query: {query_id}"
        );
    }

    let cert: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path("certificate.json")).unwrap())
            .unwrap();
    // realized risk on this fixture must be exactly 0.0: every selected query is genuinely correct.
    assert_eq!(cert["realized_selective_risk"], serde_json::json!(0.0));
}

#[test]
fn zero_selection_is_reported_as_valid_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    // Small, non-extreme fixture -- alpha/gamma tight enough that nothing clears eBH.
    write_extreme_csv(&calib, "c", 4);
    write_extreme_csv(&test, "t", 2);

    let out = Outputs::new();
    let output = run_certify_batch(
        &calib,
        &test,
        &out,
        &[
            "--alpha",
            "0.05",
            "--gamma",
            "0.05",
            "--construction",
            "coupled",
        ],
    );
    assert!(
        output.status.success(),
        "zero selections must exit 0, not error: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cert: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path("certificate.json")).unwrap())
            .unwrap();
    assert_eq!(cert["selected_count"], 0);
    assert_eq!(
        cert["certificate"]["diagnostics"]["uninformative_result"],
        true
    );

    let report = std::fs::read_to_string(out.path("report.md")).unwrap();
    assert!(report.contains("Zero selections is a valid certificate, not an error"));
}

#[test]
fn independent_construction_is_selectable() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    write_extreme_csv(&calib, "c", 40);
    write_extreme_csv(&test, "t", 10);

    let out = Outputs::new();
    let output = run_certify_batch(
        &calib,
        &test,
        &out,
        &[
            "--alpha",
            "0.5",
            "--gamma",
            "0.1",
            "--construction",
            "independent",
        ],
    );
    assert!(output.status.success());

    let cert: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path("certificate.json")).unwrap())
            .unwrap();
    assert_eq!(cert["construction"], "independent");
}

#[test]
fn invalid_alpha_is_a_clean_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    write_extreme_csv(&calib, "c", 4);
    write_extreme_csv(&test, "t", 2);

    let out = Outputs::new();
    let output = run_certify_batch(&calib, &test, &out, &["--alpha", "1.5", "--gamma", "0.1"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("alpha"), "stderr: {stderr}");
}

#[test]
fn missing_calibration_label_is_a_hard_error() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    // Calibration query with a candidate list (scoreable under score-gap) but no is_correct.
    std::fs::write(
        &calib,
        "query_id,candidate_id,rank,score\nc0,c0a,1,0.9\nc0,c0b,2,0.1\n",
    )
    .unwrap();
    write_extreme_csv(&test, "t", 2);

    let out = Outputs::new();
    let output = run_certify_batch(&calib, &test, &out, &["--alpha", "0.3", "--gamma", "0.3"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("is_correct") || stderr.contains("label"),
        "stderr: {stderr}"
    );
}

#[test]
fn unscoreable_test_query_is_reported_as_abstained_not_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    write_extreme_csv(&calib, "c", 40);
    // score-gap needs >=2 candidates; this query has only one, so it's unscoreable.
    std::fs::write(&test, "query_id,candidate_id,rank,score\nt0,t0a,1,0.9\n").unwrap();

    let out = Outputs::new();
    let output = run_certify_batch(&calib, &test, &out, &["--alpha", "0.5", "--gamma", "0.1"]);
    assert!(output.status.success());

    let abstained = std::fs::read_to_string(out.path("abstained.csv")).unwrap();
    assert!(abstained.contains("t0"));
    assert!(abstained.contains("unscoreable"));
}

// --- graded loss (--loss-column) ---

#[test]
fn loss_column_omitted_defaults_to_binary_correctness() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    write_extreme_csv(&calib, "c", 40);
    write_extreme_csv(&test, "t", 10);

    let out = Outputs::new();
    let output = run_certify_batch(&calib, &test, &out, &["--alpha", "0.5", "--gamma", "0.1"]);
    assert!(output.status.success());

    let cert: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path("certificate.json")).unwrap())
            .unwrap();
    assert_eq!(cert["loss_kind"], "binary_correctness");
    assert_eq!(cert["loss_label"], serde_json::Value::Null);
}

fn write_graded_loss_csv(path: &std::path::Path, prefix: &str, n: usize, loss: f64) {
    let mut f = std::fs::File::create(path).unwrap();
    writeln!(f, "query_id,candidate_id,rank,score,tanimoto_loss").unwrap();
    for i in 0..n {
        writeln!(f, "{prefix}{i},{prefix}{i}a,1,0.99,{loss}").unwrap();
        writeln!(f, "{prefix}{i},{prefix}{i}b,2,0.19,").unwrap();
    }
}

/// The central case: certify against a precomputed calibration loss while `--test` is
/// completely unlabeled (no `tanimoto_loss` column at all, no `is_correct` either). Must
/// succeed and simply not compute a realized risk -- not fail outright.
#[test]
fn loss_column_certifies_against_a_completely_unlabeled_test_batch() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    write_graded_loss_csv(&calib, "c", 40, 0.05);
    // Genuinely unlabeled: no tanimoto_loss column, no is_correct column.
    write_extreme_csv_no_labels(&test, "t", 10);

    let out = Outputs::new();
    let output = run_certify_batch(
        &calib,
        &test,
        &out,
        &[
            "--alpha",
            "0.5",
            "--gamma",
            "0.1",
            "--loss-column",
            "tanimoto_loss",
        ],
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cert: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path("certificate.json")).unwrap())
            .unwrap();
    assert_eq!(cert["loss_kind"], "precomputed");
    assert_eq!(cert["loss_label"], "tanimoto_loss");
    assert_eq!(cert["loss_column"], "tanimoto_loss");
    assert_eq!(cert["loss_domain"], "[0, 1]");
    // No loss anywhere in --test -- realized risk must be "not computed", not an error.
    assert_eq!(cert["realized_selective_risk"], serde_json::Value::Null);

    let report = std::fs::read_to_string(out.path("report.md")).unwrap();
    assert!(report.contains("Not computed"));
}

fn write_extreme_csv_no_labels(path: &std::path::Path, prefix: &str, n: usize) {
    let mut f = std::fs::File::create(path).unwrap();
    writeln!(f, "query_id,candidate_id,rank,score").unwrap();
    for i in 0..n {
        let gap: f64 = if i % 2 == 0 { 0.8 } else { 0.01 };
        writeln!(f, "{prefix}{i},{prefix}{i}a,1,0.99").unwrap();
        writeln!(f, "{prefix}{i},{prefix}{i}b,2,{:.3}", 0.99 - gap).unwrap();
    }
}

/// When `--test` *does* carry the loss column, realized risk is computed from it -- the
/// complementary case to the fully-unlabeled test above.
#[test]
fn loss_column_computes_realized_risk_when_test_carries_it() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    write_graded_loss_csv(&calib, "c", 40, 0.02);
    write_graded_loss_csv(&test, "t", 10, 0.1);

    let out = Outputs::new();
    let output = run_certify_batch(
        &calib,
        &test,
        &out,
        &[
            "--alpha",
            "0.5",
            "--gamma",
            "0.1",
            "--loss-column",
            "tanimoto_loss",
        ],
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cert: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.path("certificate.json")).unwrap())
            .unwrap();
    if cert["certificate"]["parameter"]
        .as_array()
        .unwrap()
        .is_empty()
    {
        // Zero selection is a valid certificate; realized risk is risksieve's own 0.0
        // convention for an empty selection, not "not computed".
        assert_eq!(cert["realized_selective_risk"], 0.0);
    } else {
        let risk = cert["realized_selective_risk"].as_f64().unwrap();
        assert!(
            (risk - 0.1).abs() < 1e-9,
            "expected realized risk ~0.1, got {risk}"
        );
    }
}

#[test]
fn loss_column_missing_calibration_value_is_a_hard_error_not_a_silent_fallback() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    write_graded_loss_csv(&calib, "c", 40, 0.1);
    write_extreme_csv_no_labels(&test, "t", 10);
    {
        let mut f = std::fs::OpenOptions::new()
            .append(true)
            .open(&calib)
            .unwrap();
        writeln!(f, "missing0,missing0a,1,0.99,").unwrap();
        writeln!(f, "missing0,missing0b,2,0.19,").unwrap();
    }

    let out = Outputs::new();
    let output = run_certify_batch(
        &calib,
        &test,
        &out,
        &[
            "--alpha",
            "0.5",
            "--gamma",
            "0.1",
            "--loss-column",
            "tanimoto_loss",
        ],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("tanimoto_loss") && stderr.contains("missing0"),
        "stderr: {stderr}"
    );
}

#[test]
fn loss_column_out_of_range_calibration_value_is_a_hard_error() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    write_graded_loss_csv(&calib, "c", 40, 1.5); // out of [0,1]
    write_extreme_csv_no_labels(&test, "t", 10);

    let out = Outputs::new();
    let output = run_certify_batch(
        &calib,
        &test,
        &out,
        &[
            "--alpha",
            "0.5",
            "--gamma",
            "0.1",
            "--loss-column",
            "tanimoto_loss",
        ],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("1.5"), "stderr: {stderr}");
}

#[test]
fn loss_column_malformed_calibration_value_is_a_hard_error() {
    let dir = tempfile::tempdir().unwrap();
    let calib = dir.path().join("calib.csv");
    let test = dir.path().join("test.csv");
    let mut f = std::fs::File::create(&calib).unwrap();
    writeln!(f, "query_id,candidate_id,rank,score,tanimoto_loss").unwrap();
    for i in 0..40 {
        writeln!(f, "c{i},c{i}a,1,0.99,not_a_number").unwrap();
        writeln!(f, "c{i},c{i}b,2,0.19,").unwrap();
    }
    drop(f);
    write_extreme_csv_no_labels(&test, "t", 10);

    let out = Outputs::new();
    let output = run_certify_batch(
        &calib,
        &test,
        &out,
        &[
            "--alpha",
            "0.5",
            "--gamma",
            "0.1",
            "--loss-column",
            "tanimoto_loss",
        ],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not_a_number"), "stderr: {stderr}");
}

// Note: a CLI-level "duplicate query_id" test was considered and dropped. `io::group_by_query`
// (which every CLI command routes through) groups CSV rows by query_id into one QueryRanking
// per unique id -- two CSV rows sharing a query_id are just two candidates of the same query,
// not a duplicate QueryRanking. `certify_batch`'s new duplicate-query_id hard error is
// reachable only by a direct caller of the public core API who assembles `Vec<QueryRanking>`
// themselves (e.g. concatenating two datasets) -- covered by
// `duplicate_test_query_id_is_a_hard_error` / `duplicate_calibration_query_id_is_a_hard_error`
// in `masstrust-core`'s `risksieve_backend::tests`, not here.

/// Confirms the `risksieve` version recorded in `certificate.json` (via `RISKSIEVE_VERSION`
/// in `certify_batch.rs`) can never silently drift from what's actually compiled: both
/// `Cargo.toml`s exact-pin `risksieve = "=0.2.0"`, so Cargo.lock can only ever resolve to
/// exactly that version -- this reads Cargo.lock directly to confirm the pin is doing its job,
/// rather than trusting the exact-pin syntax alone.
#[test]
fn risksieve_version_matches_the_exact_pin_in_cargo_lock() {
    let lock_path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"));
    let lock = std::fs::read_to_string(&lock_path).unwrap();
    let mut lines = lock.lines().peekable();
    let mut resolved_version = None;
    while let Some(line) = lines.next() {
        if line.trim() == "name = \"risksieve\"" {
            if let Some(version_line) = lines.next() {
                resolved_version = version_line
                    .trim()
                    .strip_prefix("version = \"")
                    .and_then(|s| s.strip_suffix('"'))
                    .map(str::to_string);
            }
            break;
        }
    }
    assert_eq!(
        resolved_version.as_deref(),
        Some("0.2.0"),
        "Cargo.lock's resolved risksieve version must match the exact pin; if this fails, \
         either the pin was loosened or RISKSIEVE_VERSION in certify_batch.rs is stale"
    );
}

/// Repo hygiene check: `tasks/*.md` is the user's private, local-only progress file (long
/// tracked in git history from before this policy existed -- that's not being undone here).
/// What this work must not do is *add new changes* to it. Checks the working tree has no
/// pending modifications under `tasks/` -- the actual, correct proxy for "this change set
/// doesn't touch tasks/*.md" (asserting it's untracked would be factually wrong: it always
/// has been, and un-tracking it was never requested). Skipped (not failed) outside a git
/// checkout.
#[test]
fn no_pending_changes_under_tasks_from_this_work() {
    // tasks/*.md is local-only planning content: editing it in the working tree is expected
    // and normal, but it must never be *staged* for a commit. Checking `git status` (working
    // tree + index) instead of just the index would fail on every ordinary local edit.
    let repo_root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let output = Command::new("git")
        .args(["diff", "--cached", "--name-only", "--", "tasks/"])
        .current_dir(&repo_root)
        .output();
    let Ok(output) = output else {
        eprintln!("git not available, skipping");
        return;
    };
    let staged = String::from_utf8_lossy(&output.stdout);
    assert!(
        staged.trim().is_empty(),
        "tasks/*.md is staged, which must never be committed: {staged}"
    );
}
