#!/usr/bin/env python3
"""One runnable check for this pipeline: validate -> evaluate -> report,
against a tiny synthetic fixture (fixtures/{val,test}_predictions.csv).

Does NOT touch massspecgym/torch/rdkit or the real dataset — it only
exercises validate_predictions.py, generate_report.py, and the masstrust
CLI itself. Run this before ever pointing the pipeline at the real
~231k-spectrum dataset.

Usage:
    python smoke_test.py
"""
import csv
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIXTURES = HERE / "fixtures"


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(HERE / args[0])] + list(args[1:]),
        capture_output=True,
        text=True,
    )


def write_variant(src: Path, dst: Path, mutate) -> None:
    with open(src, newline="") as f:
        rows = list(csv.DictReader(f))
    rows = mutate(rows)
    with open(dst, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)


def main() -> None:
    good_val = FIXTURES / "val_predictions.csv"
    good_test = FIXTURES / "test_predictions.csv"

    print("1. validate_predictions.py accepts the clean fixture...")
    result = run("validate_predictions.py", "--val", str(good_val), "--test", str(good_test))
    assert result.returncode == 0, f"expected success, got:\n{result.stderr}"
    print("   OK")

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)

        print("2. validate_predictions.py rejects a duplicate rank...")
        bad = tmp / "dup_rank.csv"
        write_variant(good_test, bad, lambda rows: rows + [dict(rows[0])])
        result = run("validate_predictions.py", "--val", str(good_val), "--test", str(bad))
        assert result.returncode != 0, "expected failure on duplicate rank"
        assert "duplicate rank" in result.stderr
        print("   OK")

        print("3. validate_predictions.py rejects a non-finite score...")
        bad = tmp / "nonfinite_score.csv"

        def inject_nan(rows):
            rows = [dict(r) for r in rows]
            rows[0]["score"] = "nan"
            return rows

        write_variant(good_test, bad, inject_nan)
        result = run("validate_predictions.py", "--val", str(good_val), "--test", str(bad))
        assert result.returncode != 0, "expected failure on non-finite score"
        assert "non-finite" in result.stderr
        print("   OK")

        print("4. validate_predictions.py rejects a query with no true candidate...")
        bad = tmp / "no_true_candidate.csv"

        def strip_true(rows):
            rows = [dict(r) for r in rows]
            for r in rows:
                if r["query_id"] == "t1":
                    r["is_correct"] = "false"
            return rows

        write_variant(good_test, bad, strip_true)
        result = run("validate_predictions.py", "--val", str(good_val), "--test", str(bad))
        assert result.returncode != 0, "expected failure on missing true candidate"
        assert "no true candidate" in result.stderr
        print("   OK")

    print("5. generate_report.py runs end to end on the clean fixture...")
    with tempfile.TemporaryDirectory() as tmp:
        out_dir = Path(tmp) / "report"
        result = run(
            "generate_report.py",
            "--val", str(good_val),
            "--test", str(good_test),
            "--out-dir", str(out_dir),
            "--bootstrap", "200",
        )
        assert result.returncode == 0, f"expected success, got:\n{result.stderr}"
        report_csv = out_dir / "report.csv"
        report_md = out_dir / "report.md"
        assert report_csv.exists(), "report.csv was not written"
        assert report_md.exists(), "report.md was not written"

        with open(report_csv, newline="") as f:
            rows = list(csv.DictReader(f))
        assert len(rows) == 4 * 3, f"expected 4 methods x 3 target risks, got {len(rows)}"
        for row in rows:
            coverage = float(row["test_coverage_achieved"])
            assert 0.0 <= coverage <= 1.0, f"coverage out of range: {row}"
            if row["method"] == "candidate-count":
                assert float(row["unscoreable_rate"]) == 0.0, "candidate-count must always be scoreable"
    print("   OK")

    print("\nAll smoke tests passed.")


if __name__ == "__main__":
    main()
