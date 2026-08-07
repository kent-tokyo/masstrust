#!/usr/bin/env python3
"""Run all 5 methods x 3 pre-registered alphas on the Selective-MSMS external prediction split,
and collect the metrics required by the benchmark's reporting contract (see ../README.md).

Methods (all operate on the SAME `max-prob` confidence score -- see convert_to_masstrust_csv.py):
  - empirical / binomial / legacy-crc: masstrust calibrate (on calibration.csv) + evaluate
    (on evaluation.csv), masstrust's existing threshold-policy machinery.
  - risksieve SCoRE-SDR coupled / independent: masstrust certify-batch, jointly over
    calibration.csv + evaluation.csv (its guarantee is about the joint draw of both, not a
    reusable threshold -- see docs/risksieve-integration.md).

alpha/gamma and the split are fixed before this script is run (split_manifest.json is
immutable; alpha in {0.01, 0.05, 0.10} was specified as a precondition of this benchmark, not
chosen after seeing results). This script does not adjust either based on its own output.
"""
import json
import os
import subprocess
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..", "..")
BIN = os.path.join(ROOT, "target", "release", "masstrust")
DATA_DIR = os.path.join(HERE, "..", "data")
RUNS_DIR = os.path.join(HERE, "..", "results", "runs")
CAL_CSV = os.path.join(DATA_DIR, "calibration.csv")
EVAL_CSV = os.path.join(DATA_DIR, "evaluation.csv")

ALPHAS = [0.01, 0.05, 0.10]
BINOMIAL_CONFIDENCE_LEVEL = 0.95  # pre-registered alongside alpha/gamma, not swept


def run(cmd):
    t0 = time.time()
    r = subprocess.run(cmd, capture_output=True, text=True)
    elapsed = time.time() - t0
    return r, elapsed


def threshold_method(method, alpha, run_dir):
    os.makedirs(run_dir, exist_ok=True)
    policy_path = os.path.join(run_dir, "policy.json")
    report_path = os.path.join(run_dir, "eval_report.json")

    cal_cmd = [BIN, "calibrate", CAL_CSV, "--score", "max-prob", "--error-rate", str(alpha),
               "--method", method, "--out", policy_path]
    if method == "binomial":
        cal_cmd += ["--confidence-level", str(BINOMIAL_CONFIDENCE_LEVEL)]
    cal_r, cal_t = run(cal_cmd)

    eval_cmd = [BIN, "evaluate", EVAL_CSV, "--policy", policy_path, "--out", report_path]
    eval_r, eval_t = run(eval_cmd)

    result = {
        "method": method, "alpha": alpha, "construction": None,
        "runtime_seconds": round(cal_t + eval_t, 4),
        "calibrate_stderr": cal_r.stderr, "evaluate_stderr": eval_r.stderr,
        "calibrate_returncode": cal_r.returncode, "evaluate_returncode": eval_r.returncode,
    }
    if eval_r.returncode == 0 and os.path.exists(report_path):
        with open(report_path) as f:
            result["report"] = json.load(f)
    return result


def sdr_method(construction, alpha, run_dir):
    os.makedirs(run_dir, exist_ok=True)
    accepted_path = os.path.join(run_dir, "accepted.csv")
    abstained_path = os.path.join(run_dir, "abstained.csv")
    certificate_path = os.path.join(run_dir, "certificate.json")
    report_path = os.path.join(run_dir, "report.md")

    cmd = [BIN, "certify-batch", "--calibration", CAL_CSV, "--test", EVAL_CSV,
           "--score", "max-prob", "--alpha", str(alpha), "--gamma", str(alpha),
           "--construction", construction,
           "--accepted", accepted_path, "--abstained", abstained_path,
           "--certificate", certificate_path, "--report", report_path]
    r, t = run(cmd)

    result = {
        "method": f"risksieve_sdr_{construction}", "alpha": alpha, "construction": construction,
        "runtime_seconds": round(t, 4),
        "stderr": r.stderr, "returncode": r.returncode,
    }
    if r.returncode == 0 and os.path.exists(certificate_path):
        with open(certificate_path) as f:
            result["certificate"] = json.load(f)
    return result


def main():
    os.makedirs(RUNS_DIR, exist_ok=True)
    all_results = []
    for alpha in ALPHAS:
        for method in ["empirical", "binomial", "crc"]:
            run_dir = os.path.join(RUNS_DIR, f"alpha_{alpha}", method)
            res = threshold_method(method, alpha, run_dir)
            all_results.append(res)
            print(f"[{method} alpha={alpha}] rc={res['calibrate_returncode']}/{res['evaluate_returncode']}"
                  f" coverage={res.get('report', {}).get('coverage')}"
                  f" risk={res.get('report', {}).get('risk')}")
        for construction in ["coupled", "independent"]:
            run_dir = os.path.join(RUNS_DIR, f"alpha_{alpha}", f"sdr_{construction}")
            res = sdr_method(construction, alpha, run_dir)
            all_results.append(res)
            cert = res.get("certificate", {})
            print(f"[sdr_{construction} alpha={alpha}] rc={res['returncode']}"
                  f" selected={cert.get('selected_count')}"
                  f" realized_risk={cert.get('realized_selective_risk')}")

    summary_path = os.path.join(HERE, "..", "results", "comparison_raw.json")
    with open(summary_path, "w") as f:
        json.dump(all_results, f, indent=2)
    print(f"\nwrote {summary_path}")


if __name__ == "__main__":
    main()
