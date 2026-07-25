#!/usr/bin/env python3
"""Data-quality gate for exported prediction CSVs (masstrust candidate schema).

Hard-fails (non-zero exit) on:
  - duplicate `rank` within a query
  - a query with no `is_correct == True` candidate (missing ground truth)
  - non-finite `score` values
  - cross-split leakage between the val and test CSVs (query_id/inchikey/formula
    overlap) — delegated to `masstrust validate-split`, not reimplemented here.

Usage:
    python validate_predictions.py --val data/val_predictions.csv --test data/test_predictions.csv
"""
import argparse
import math
import shutil
import subprocess
import sys
from pathlib import Path


def check_csv(path: Path) -> list[str]:
    import csv

    errors = []
    by_query_ranks: dict[str, set[int]] = {}
    has_true: dict[str, bool] = {}

    with open(path, newline="") as f:
        reader = csv.DictReader(f)
        for i, row in enumerate(reader):
            qid = row["query_id"]
            rank = int(row["rank"])
            ranks = by_query_ranks.setdefault(qid, set())
            if rank in ranks:
                errors.append(f"{path.name}: duplicate rank {rank} for query {qid} (row {i})")
            ranks.add(rank)

            try:
                score = float(row["score"])
            except ValueError:
                errors.append(f"{path.name}: non-numeric score {row['score']!r} (row {i}, query {qid})")
                score = float("nan")
            if not math.isfinite(score):
                errors.append(f"{path.name}: non-finite score {row['score']!r} (row {i}, query {qid})")

            is_correct = row["is_correct"].strip().lower() == "true"
            has_true[qid] = has_true.get(qid, False) or is_correct

    for qid, found in has_true.items():
        if not found:
            errors.append(f"{path.name}: query {qid} has no true candidate in its pool")

    return errors


def find_masstrust_binary() -> str:
    exe = shutil.which("masstrust")
    if exe:
        return exe
    # Fall back to a workspace debug build, so this validator works right
    # after `cargo build` without requiring `cargo install`.
    repo_root = Path(__file__).resolve().parents[2]
    candidate = repo_root / "target" / "debug" / "masstrust"
    if candidate.exists():
        return str(candidate)
    sys.exit(
        "masstrust binary not found on PATH or at target/debug/masstrust. "
        "Run `cargo build` at the repo root, or `cargo install --path crates/masstrust-cli`."
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--val", type=Path, required=True)
    parser.add_argument("--test", type=Path, required=True)
    args = parser.parse_args()

    errors = check_csv(args.val) + check_csv(args.test)

    masstrust = find_masstrust_binary()
    result = subprocess.run(
        [
            masstrust,
            "validate-split",
            "--calibration",
            str(args.val),
            "--test",
            str(args.test),
        ],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        errors.append(f"masstrust validate-split failed:\n{result.stdout}{result.stderr}")

    if errors:
        print("VALIDATION FAILED:", file=sys.stderr)
        for e in errors:
            print(f"  - {e}", file=sys.stderr)
        sys.exit(1)

    print(f"OK: {args.val.name} and {args.test.name} passed all validation checks.")


if __name__ == "__main__":
    main()
