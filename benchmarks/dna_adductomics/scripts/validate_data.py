#!/usr/bin/env python3
"""Wrap `masstrust validate-split` (compound-disjoint leakage check) and a schema sanity check.

Hard-fails (non-zero exit) on any query_id overlap between calibration.csv/test.csv -- with
only 8 distinct compounds total (FEASIBILITY.md ss2.3), even one leaked compound would be a
large fraction of the test set, not a rounding error.
"""
import argparse
import csv
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.join(HERE, "..", "..", "..")
REQUIRED_COLUMNS = {
    "query_id", "candidate_id", "rank", "score", "is_correct",
    "ground_truth_tier", "candidate_origin", "run_kind",
}


def check_schema(path):
    with open(path, newline="") as f:
        header = set(next(csv.reader(f)))
    missing = REQUIRED_COLUMNS - header
    if missing:
        raise SystemExit(f"{path}: missing expected columns: {sorted(missing)}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data-dir", default=os.path.join(HERE, "..", "data"))
    args = ap.parse_args()

    calib = os.path.abspath(os.path.join(args.data_dir, "calibration.csv"))
    test = os.path.abspath(os.path.join(args.data_dir, "test.csv"))
    check_schema(calib)
    check_schema(test)

    out_json = os.path.abspath(os.path.join(args.data_dir, "validate_split_report.json"))
    result = subprocess.run(
        [
            "cargo", "run", "-q", "-p", "masstrust-cli", "--",
            "validate-split", "--calibration", calib, "--test", test, "--out", out_json,
        ],
        cwd=REPO_ROOT,
    )
    if result.returncode != 0:
        print("masstrust validate-split reported hard-failing leakage (query_id overlap) "
              "-- see output above. This is a real failure, not a formatting issue.",
              file=sys.stderr)
        sys.exit(result.returncode)
    print(f"validate-split passed; report at {out_json}")


if __name__ == "__main__":
    main()
