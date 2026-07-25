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
