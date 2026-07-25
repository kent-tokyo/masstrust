use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    let mut p = std::env::current_exe().unwrap();
    // tests run from target/debug/deps/cli-<hash>
    p.pop();
    p.pop();
    p.push("masstrust");
    p
}

fn examples_dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples"))
}

#[test]
fn test_curve_command() {
    let out = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new(bin())
        .args([
            "curve",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--score",
            "score-gap",
            "--out",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("failed to run masstrust");
    assert!(status.success());
    assert!(out.path().metadata().unwrap().len() > 0);
}

#[test]
fn test_calibrate_empirical() {
    let out = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new(bin())
        .args([
            "calibrate",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--score",
            "score-gap",
            "--error-rate",
            "0.05",
            "--method",
            "empirical",
            "--out",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("failed to run masstrust");
    assert!(status.success());

    let content = std::fs::read_to_string(out.path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(v["version"], "0.1.0");
    assert_eq!(v["scoring_method"], "score_gap");
}

#[test]
fn test_calibrate_binomial() {
    let out = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new(bin())
        .args([
            "calibrate",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--score",
            "score-gap",
            "--error-rate",
            "0.05",
            "--method",
            "binomial",
            "--confidence-level",
            "0.95",
            "--out",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("failed to run masstrust");
    assert!(status.success());
}

#[test]
fn test_apply_command() {
    let policy_file = tempfile::NamedTempFile::new().unwrap();
    // First calibrate
    Command::new(bin())
        .args([
            "calibrate",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--score",
            "score-gap",
            "--error-rate",
            "0.20",
            "--method",
            "empirical",
            "--out",
            policy_file.path().to_str().unwrap(),
        ])
        .status()
        .unwrap();

    let trusted = tempfile::NamedTempFile::new().unwrap();
    let abstained = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new(bin())
        .args([
            "apply",
            examples_dir().join("candidates.csv").to_str().unwrap(),
            "--policy",
            policy_file.path().to_str().unwrap(),
            "--out",
            trusted.path().to_str().unwrap(),
            "--abstained",
            abstained.path().to_str().unwrap(),
        ])
        .status()
        .expect("failed to run masstrust");
    assert!(status.success());
}

#[test]
fn test_missing_column_error() {
    use std::io::Write;
    let mut bad_csv = tempfile::NamedTempFile::new().unwrap();
    write!(bad_csv, "query_id,candidate_id,rank\nq1,c1,1\n").unwrap();
    let out = tempfile::NamedTempFile::new().unwrap();

    let output = Command::new(bin())
        .args([
            "curve",
            bad_csv.path().to_str().unwrap(),
            "--score",
            "score-gap",
            "--out",
            out.path().to_str().unwrap(),
        ])
        .output()
        .expect("failed to run masstrust");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("score") || stderr.contains("Missing"));
}

#[test]
fn test_unknown_scoring_method_error() {
    let out = tempfile::NamedTempFile::new().unwrap();
    let output = Command::new(bin())
        .args([
            "curve",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--score",
            "invalid-method",
            "--out",
            out.path().to_str().unwrap(),
        ])
        .output()
        .expect("failed to run masstrust");
    assert!(!output.status.success());
}

#[test]
fn test_compare_command() {
    let out = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new(bin())
        .args([
            "compare",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--scores",
            "score-gap,candidate-count",
            "--error-rate",
            "0.05",
            "--method",
            "empirical",
            "--out",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("failed to run masstrust");
    assert!(status.success());
    assert!(out.path().metadata().unwrap().len() > 0);
}

#[test]
fn test_drift_command() {
    let out = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new(bin())
        .args([
            "drift",
            "--calibration",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--new",
            examples_dir().join("candidates.csv").to_str().unwrap(),
            "--score",
            "score-gap",
            "--out",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("failed to run masstrust");
    assert!(status.success());
    let content = std::fs::read_to_string(out.path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(["low", "medium", "high"].contains(&v["warning"].as_str().unwrap()));
    assert!(v["n_calibration"].as_u64().unwrap() > 0);
}

#[test]
fn test_evaluate_command() {
    let policy_file = tempfile::NamedTempFile::new().unwrap();
    // Calibrate on one labeled set (stands in for a validation split)...
    Command::new(bin())
        .args([
            "calibrate",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--score",
            "score-gap",
            "--error-rate",
            "0.20",
            "--method",
            "empirical",
            "--out",
            policy_file.path().to_str().unwrap(),
        ])
        .status()
        .unwrap();

    // ...then evaluate the fixed threshold on a different labeled set (stands in for test).
    let out = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new(bin())
        .args([
            "evaluate",
            examples_dir()
                .join("massspecgym_candidates.csv")
                .to_str()
                .unwrap(),
            "--policy",
            policy_file.path().to_str().unwrap(),
            "--out",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("failed to run masstrust");
    assert!(status.success());

    let content = std::fs::read_to_string(out.path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(v["total"].as_u64().unwrap() > 0);
    assert!(v["coverage"].as_f64().unwrap() >= 0.0);
}

#[test]
fn test_evaluate_rejects_grouped_policy() {
    let policy_file = tempfile::NamedTempFile::new().unwrap();
    Command::new(bin())
        .args([
            "calibrate",
            examples_dir()
                .join("labeled_candidates_grouped.csv")
                .to_str()
                .unwrap(),
            "--score",
            "score-gap",
            "--error-rate",
            "0.20",
            "--method",
            "empirical",
            "--group-col",
            "adduct",
            "--out",
            policy_file.path().to_str().unwrap(),
        ])
        .status()
        .unwrap();

    let out = tempfile::NamedTempFile::new().unwrap();
    let output = Command::new(bin())
        .args([
            "evaluate",
            examples_dir()
                .join("labeled_candidates_grouped.csv")
                .to_str()
                .unwrap(),
            "--policy",
            policy_file.path().to_str().unwrap(),
            "--out",
            out.path().to_str().unwrap(),
        ])
        .output()
        .expect("failed to run masstrust");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("grouped"));
}

#[test]
fn test_validate_split_no_overlap() {
    // labeled_candidates uses q1-q4, candidates uses q5-q6 — no overlap
    let output = Command::new(bin())
        .args([
            "validate-split",
            "--calibration",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--test",
            examples_dir().join("candidates.csv").to_str().unwrap(),
        ])
        .output()
        .expect("failed to run masstrust");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No overlap detected"));
}

#[test]
fn test_validate_split_overlap_exits_1() {
    use std::io::Write;
    let mut f = tempfile::NamedTempFile::new().unwrap();
    write!(
        f,
        "query_id,candidate_id,rank,score\nq1,c1,1,0.9\nq1,c2,2,0.8\n"
    )
    .unwrap();
    let output = Command::new(bin())
        .args([
            "validate-split",
            "--calibration",
            f.path().to_str().unwrap(),
            "--test",
            f.path().to_str().unwrap(),
        ])
        .output()
        .expect("failed to run masstrust");
    assert!(!output.status.success()); // exit 1
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("WARNING"));
}

#[test]
fn test_validate_split_pool_overlap_only_exits_0() {
    // Disjoint query_ids (qc1 vs qt1), but candidate "mol_a" recurs in both pools.
    // Pool overlap alone must not be a hard failure.
    use std::io::Write;
    let mut cal = tempfile::NamedTempFile::new().unwrap();
    write!(
        cal,
        "query_id,candidate_id,rank,score\nqc1,mol_a,1,0.9\nqc1,mol_b,2,0.5\n"
    )
    .unwrap();
    let mut tst = tempfile::NamedTempFile::new().unwrap();
    write!(
        tst,
        "query_id,candidate_id,rank,score\nqt1,mol_a,1,0.9\nqt1,mol_c,2,0.5\n"
    )
    .unwrap();

    let out = tempfile::NamedTempFile::new().unwrap();
    let output = Command::new(bin())
        .args([
            "validate-split",
            "--calibration",
            cal.path().to_str().unwrap(),
            "--test",
            tst.path().to_str().unwrap(),
            "--out",
            out.path().to_str().unwrap(),
        ])
        .output()
        .expect("failed to run masstrust");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("candidate pool overlap: 1"));
    assert!(stdout.contains("No overlap detected"));

    let content = std::fs::read_to_string(out.path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(v["query_id_overlap"].as_u64().unwrap(), 0);
    assert_eq!(v["candidate_pool_overlap"].as_u64().unwrap(), 1);
    assert!(!v["hard_failure"].as_bool().unwrap());
}

#[test]
fn test_evaluate_bootstrap_ci() {
    let policy_file = tempfile::NamedTempFile::new().unwrap();
    Command::new(bin())
        .args([
            "calibrate",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--score",
            "score-gap",
            "--error-rate",
            "0.20",
            "--method",
            "empirical",
            "--out",
            policy_file.path().to_str().unwrap(),
        ])
        .status()
        .unwrap();

    let out = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new(bin())
        .args([
            "evaluate",
            examples_dir()
                .join("massspecgym_candidates.csv")
                .to_str()
                .unwrap(),
            "--policy",
            policy_file.path().to_str().unwrap(),
            "--bootstrap",
            "200",
            "--out",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("failed to run masstrust");
    assert!(status.success());

    let content = std::fs::read_to_string(out.path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();
    let coverage = v["coverage"].as_f64().unwrap();
    let lo = v["coverage_ci_lo"].as_f64().unwrap();
    let hi = v["coverage_ci_hi"].as_f64().unwrap();
    assert!(
        lo <= coverage && coverage <= hi,
        "coverage {coverage} not in [{lo}, {hi}]"
    );
    assert!(v["risk_ci_n"].as_u64().unwrap() <= 200);
}

#[test]
fn test_evaluate_abstain_all() {
    use std::io::Write;
    // A threshold no confidence value can ever reach forces 0 acceptances.
    let mut policy_file = tempfile::NamedTempFile::new().unwrap();
    write!(
        policy_file,
        r#"{{"version":"0.1.0","scoring_method":"score_gap","threshold":1000000.0,
            "target_error_rate":0.05,"calibration_method":"empirical",
            "confidence_level":null,"created_by":"test"}}"#
    )
    .unwrap();

    let out = tempfile::NamedTempFile::new().unwrap();
    let status = Command::new(bin())
        .args([
            "evaluate",
            examples_dir()
                .join("labeled_candidates.csv")
                .to_str()
                .unwrap(),
            "--policy",
            policy_file.path().to_str().unwrap(),
            "--out",
            out.path().to_str().unwrap(),
        ])
        .status()
        .expect("failed to run masstrust");
    assert!(status.success());

    let content = std::fs::read_to_string(out.path()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(v["accepted"].as_u64().unwrap(), 0);
    assert!(v["abstain_all"].as_bool().unwrap());
    assert!(v["abstain_reason"].as_str().unwrap().contains("0/"));
    assert!(v["risk"].is_null());
}
