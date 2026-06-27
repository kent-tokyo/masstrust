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
