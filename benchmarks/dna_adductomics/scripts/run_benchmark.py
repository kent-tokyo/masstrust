#!/usr/bin/env python3
"""Run masstrust's own confidence-scoring / calibration / evaluation machinery against the real
calibration.csv / test.csv pair (see ../README.md protocol steps 5-8).

**Preflight, not a benchmark** -- see ../FEASIBILITY.md ss2.4. With 4 calibration / 4 test
compounds, every number here carries a huge Wilson interval; `generate_report.py` prints that
plainly rather than a single headline. matchms cosine similarity is a bare score, not a
calibrated probability, so only the four score-only methods are run (`score-gap`, `score-ratio`,
`topk-gap`, `candidate-count`) -- same caveat, same reasoning, as
`benchmarks/massspecgym/README.md`'s "Output schema" section (`probability` is never populated
here either).
"""
import argparse
import json
import os
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
MASSTRUST_BIN = os.path.join(REPO_ROOT, "target", "release", "masstrust")

SCORE_METHODS = ["score-gap", "score-ratio", "topk-gap", "candidate-count"]
ERROR_RATES = [0.05, 0.10, 0.20]
CAL_METHODS = ["empirical", "binomial"]


def run(args_list):
    result = subprocess.run([MASSTRUST_BIN] + args_list, capture_output=True, text=True)
    return result.returncode, result.stdout, result.stderr


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data-dir", default=os.path.join(HERE, "..", "data"))
    ap.add_argument("--out-dir", default=os.path.join(HERE, "..", "report"))
    args = ap.parse_args()
    os.makedirs(args.out_dir, exist_ok=True)
    runs_dir = os.path.join(args.out_dir, "runs")
    os.makedirs(runs_dir, exist_ok=True)

    if not os.path.exists(MASSTRUST_BIN):
        raise SystemExit(f"{MASSTRUST_BIN} not found -- run "
                          "`cargo build --release -p masstrust-cli` first")

    calib = os.path.abspath(os.path.join(args.data_dir, "calibration.csv"))
    test = os.path.abspath(os.path.join(args.data_dir, "test.csv"))
    full = os.path.abspath(os.path.join(args.data_dir, "candidates.csv"))

    commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=REPO_ROOT,
                             capture_output=True, text=True).stdout.strip()
    dirty = subprocess.run(["git", "status", "--porcelain"], cwd=REPO_ROOT,
                            capture_output=True, text=True).stdout.strip() != ""
    manifest = {
        "run_kind": "preflight",
        "masstrust_commit": commit,
        "working_tree_dirty": dirty,
        "score_methods": SCORE_METHODS,
        "error_rate_targets": ERROR_RATES,
        "calibration_methods": CAL_METHODS,
        "split": "deterministic alphabetical-by-InChIKey first-half/second-half (see export_candidates.py)",
    }
    with open(os.path.join(args.out_dir, "manifest.json"), "w") as f:
        json.dump(manifest, f, indent=2)

    summary = {"compare": None, "calibrate_evaluate": []}

    # Baseline comparison across all four score-only methods, on the full 8-query set
    # (descriptive, not a calibration/test split -- exploratory only, per README.md step 6).
    compare_out = os.path.join(runs_dir, "compare.csv")
    rc, out, err = run([
        "compare", full, "--scores", ",".join(SCORE_METHODS),
        "--error-rate", "0.10", "--bootstrap", "200", "--out", compare_out,
    ])
    summary["compare"] = {"returncode": rc, "out_csv": compare_out, "stdout": out, "stderr": err}
    print("compare:", "ok" if rc == 0 else f"FAILED ({err.strip()})")

    # calibrate on calibration.csv, evaluate (held out, no recalibration) on test.csv --
    # the actual selective-prediction protocol, at every (score method, target risk,
    # calibration method) combination. n is tiny; every result is reported, including
    # "no threshold found" (binomial at n=4 calibration queries is expected to often fail
    # to clear any target -- that failure is itself the honest small-n finding).
    for score in SCORE_METHODS:
        for cal_method in CAL_METHODS:
            for target in ERROR_RATES:
                tag = f"{score}_{cal_method}_{target}"
                policy_path = os.path.join(runs_dir, f"policy_{tag}.json")
                cal_args = [
                    "calibrate", calib, "--score", score, "--error-rate", str(target),
                    "--method", cal_method, "--out", policy_path,
                ]
                if cal_method == "binomial":
                    cal_args += ["--confidence-level", "0.95"]
                rc1, out1, err1 = run(cal_args)

                entry = {
                    "score": score, "calibration_method": cal_method, "target_risk": target,
                    "calibrate_returncode": rc1, "calibrate_stdout": out1, "calibrate_stderr": err1,
                }
                if rc1 == 0 and os.path.exists(policy_path):
                    eval_out = os.path.join(runs_dir, f"eval_{tag}.json")
                    rc2, out2, err2 = run([
                        "evaluate", test, "--policy", policy_path,
                        "--out", eval_out, "--bootstrap", "200",
                    ])
                    entry.update({
                        "evaluate_returncode": rc2, "evaluate_stdout": out2, "evaluate_stderr": err2,
                        "eval_report": eval_out if rc2 == 0 else None,
                    })
                    if rc2 == 0 and os.path.exists(eval_out):
                        with open(eval_out) as f:
                            entry["eval_result"] = json.load(f)
                summary["calibrate_evaluate"].append(entry)
                status = "ok" if entry.get("evaluate_returncode") == 0 else (
                    "no-threshold" if rc1 != 0 else "eval-failed")
                print(f"{tag}: {status}")

    summary_path = os.path.join(args.out_dir, "run_summary.json")
    with open(summary_path, "w") as f:
        json.dump(summary, f, indent=2)
    print(f"wrote {summary_path}")


if __name__ == "__main__":
    main()
