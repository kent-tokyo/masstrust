#!/usr/bin/env python3
"""Assemble the masstrust-only MassSpecGym benchmark report.

Runs masstrust's own CLI (curve / compare / calibrate / evaluate) end to end:
  - calibration always on --val, evaluation always on --test (never both on
    the same queries)
  - AURC / E-AURC / bootstrap CI and unscoreable rate come from the TEST
    curve (ranking quality needs no fixed threshold)
  - achieved coverage/risk at each target come from calibrating on --val
    then evaluating that fixed policy on --test via `masstrust evaluate`

Only masstrust's four score-only methods are compared here (score-gap,
score-ratio, topk-gap, candidate-count) — max-prob/margin/entropy/effective-k
need a genuinely calibrated `probability`, which this round's predictions
don't have (see README.md). Coverage@Risk-5% on score-gap is the headline
number; everything else is secondary.

Usage:
    python generate_report.py --val data/val_predictions.csv \\
        --test data/test_predictions.csv --out-dir report --bootstrap 1000
"""
import argparse
import csv
import json
import resource
import shutil
import subprocess
import sys
import time
from pathlib import Path

METHODS = ["score-gap", "score-ratio", "topk-gap", "candidate-count"]
PRIMARY_METHOD = "score-gap"
TARGET_RISKS = [0.01, 0.05, 0.10]
PRIMARY_RISK = 0.05


def find_masstrust_binary() -> str:
    exe = shutil.which("masstrust")
    if exe:
        return exe
    repo_root = Path(__file__).resolve().parents[2]
    candidate = repo_root / "target" / "debug" / "masstrust"
    if candidate.exists():
        return str(candidate)
    sys.exit(
        "masstrust binary not found on PATH or at target/debug/masstrust. "
        "Run `cargo build` at the repo root, or `cargo install --path crates/masstrust-cli`."
    )


def run_timed(cmd: list[str]) -> tuple[subprocess.CompletedProcess, float, int]:
    start = time.perf_counter()
    result = subprocess.run(cmd, capture_output=True, text=True)
    elapsed = time.perf_counter() - start
    # ru_maxrss is the running peak across all children this process has
    # spawned so far (kilobytes on Linux, bytes on macOS) — a conservative
    # cumulative figure, not a strict per-call measurement.
    peak_rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    if result.returncode != 0:
        sys.exit(f"command failed: {' '.join(cmd)}\n{result.stdout}{result.stderr}")
    return result, elapsed, peak_rss


def read_csv_rows(path: Path) -> list[dict]:
    with open(path, newline="") as f:
        return list(csv.DictReader(f))


def top1_accuracy(test_csv: Path) -> float:
    by_query = {}
    for row in read_csv_rows(test_csv):
        if int(row["rank"]) == 1:
            by_query[row["query_id"]] = row["is_correct"].strip().lower() == "true"
    return sum(by_query.values()) / len(by_query) if by_query else float("nan")


def unscoreable_rate(curve_rows: list[dict]) -> float:
    if not curve_rows:
        return 1.0
    last = curve_rows[-1]
    total = int(last["total"])
    if total == 0:
        return 1.0
    return 1.0 - (int(last["accepted"]) / total)


def run_validate_split(masstrust: str, val_csv: Path, test_csv: Path, out_dir: Path):
    # Not a gate here (validate_predictions.py already gates the pipeline before this
    # script runs) — just captures the leakage stats for the report. `--out` is written
    # by `masstrust validate-split` even on its hard-failure exit(1) path, so the stats
    # survive either way.
    out_json = out_dir / "validate_split.json"
    subprocess.run(
        [
            masstrust, "validate-split",
            "--calibration", str(val_csv),
            "--test", str(test_csv),
            "--out", str(out_json),
        ],
        capture_output=True,
        text=True,
    )
    return json.loads(out_json.read_text()) if out_json.exists() else None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--val", type=Path, required=True)
    parser.add_argument("--test", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=None)
    parser.add_argument("--out-dir", type=Path, default=Path("report"))
    parser.add_argument("--bootstrap", type=int, default=1000)
    args = parser.parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)
    masstrust = find_masstrust_binary()

    manifest = {}
    manifest_pth = args.manifest or args.val.parent / "manifest.json"
    if manifest_pth.exists():
        manifest = json.loads(manifest_pth.read_text())

    total_elapsed = 0.0
    peak_rss = 0

    # AURC / E-AURC / bootstrap CI for all four methods on the test set, in one call.
    compare_csv = args.out_dir / "compare_test.csv"
    _, elapsed, rss = run_timed(
        [
            masstrust, "compare", str(args.test),
            "--scores", ",".join(METHODS),
            "--error-rate", str(PRIMARY_RISK),
            "--method", "empirical",
            "--bootstrap", str(args.bootstrap),
            "--out", str(compare_csv),
        ]
    )
    total_elapsed += elapsed
    peak_rss = max(peak_rss, rss)
    compare_rows = {r["method"]: r for r in read_csv_rows(compare_csv)}

    rows_out = []
    for method in METHODS:
        # Full curve on the test set, purely to derive the unscoreable rate
        # (compare's row reflects one calibrated threshold, not max coverage).
        curve_csv = args.out_dir / f"curve_test_{method}.csv"
        _, elapsed, rss = run_timed(
            [masstrust, "curve", str(args.test), "--score", method, "--out", str(curve_csv)]
        )
        total_elapsed += elapsed
        peak_rss = max(peak_rss, rss)
        unscoreable = unscoreable_rate(read_csv_rows(curve_csv))

        cmp_row = compare_rows.get(method, {})

        for target_risk in TARGET_RISKS:
            policy_json = args.out_dir / f"policy_{method}_{target_risk}.json"
            eval_json = args.out_dir / f"eval_{method}_{target_risk}.json"

            _, elapsed, rss = run_timed(
                [
                    masstrust, "calibrate", str(args.val),
                    "--score", method,
                    "--error-rate", str(target_risk),
                    "--method", "empirical",
                    "--out", str(policy_json),
                ]
            )
            total_elapsed += elapsed
            peak_rss = max(peak_rss, rss)

            _, elapsed, rss = run_timed(
                [
                    masstrust, "evaluate", str(args.test),
                    "--policy", str(policy_json),
                    "--bootstrap", str(args.bootstrap),
                    "--out", str(eval_json),
                ]
            )
            total_elapsed += elapsed
            peak_rss = max(peak_rss, rss)
            eval_result = json.loads(eval_json.read_text())

            rows_out.append(
                {
                    "method": method,
                    "target_risk": target_risk,
                    "aurc": cmp_row.get("aurc", ""),
                    "eaurc": cmp_row.get("eaurc", ""),
                    "aurc_ci_lo": cmp_row.get("aurc_ci_lo", ""),
                    "aurc_ci_hi": cmp_row.get("aurc_ci_hi", ""),
                    "unscoreable_rate": unscoreable,
                    "test_coverage_achieved": eval_result["coverage"],
                    "test_coverage_ci_lo": eval_result.get("coverage_ci_lo"),
                    "test_coverage_ci_hi": eval_result.get("coverage_ci_hi"),
                    "test_risk_achieved": eval_result["risk"],
                    "test_risk_ci_lo": eval_result.get("risk_ci_lo"),
                    "test_risk_ci_hi": eval_result.get("risk_ci_hi"),
                    "test_risk_ci_n": eval_result.get("risk_ci_n"),
                    "test_risk_wilson_upper": eval_result.get("risk_wilson_upper"),
                    "wilson_confidence_level": eval_result.get("wilson_confidence_level"),
                    "target_risk_exceeded": eval_result.get("target_risk_exceeded"),
                    "test_accepted": eval_result["accepted"],
                    "test_total": eval_result["total"],
                    "test_errors": eval_result["errors"],
                    "abstain_all": eval_result.get("abstain_all", False),
                    "abstain_reason": eval_result.get("abstain_reason"),
                }
            )

    report_csv = args.out_dir / "report.csv"
    with open(report_csv, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=list(rows_out[0].keys()))
        writer.writeheader()
        writer.writerows(rows_out)

    acc = top1_accuracy(args.test)
    # headline is looked up fresh from rows_out (not the loop's leftover `eval_result`,
    # which after the loops end holds whatever ran last — e.g. candidate-count@0.10) so
    # the printed coverage/risk numbers and their CIs always come from the same
    # score-gap@5% row.
    headline = next(
        r for r in rows_out if r["method"] == PRIMARY_METHOD and r["target_risk"] == PRIMARY_RISK
    )

    leakage = run_validate_split(masstrust, args.val, args.test, args.out_dir)

    def _ci(lo, hi, decimals=4):
        if lo is None or hi is None:
            return ""
        return f" (95% CI [{lo:.{decimals}f}, {hi:.{decimals}f}])"

    headline_coverage_ci = _ci(headline["test_coverage_ci_lo"], headline["test_coverage_ci_hi"])
    headline_risk_wilson = (
        f" (Wilson {headline['wilson_confidence_level'] * 100:.0f}% upper bound: "
        f"{headline['test_risk_wilson_upper']:.4f})"
        if headline.get("test_risk_wilson_upper") is not None
        else ""
    )

    run_kind = manifest.get("run_kind", "unknown")

    report_md = args.out_dir / "report.md"
    lines = [
        "# masstrust MassSpecGym benchmark report",
        "",
    ]
    if run_kind == "preflight":
        lines += [
            "> ⚠️ **PREFLIGHT RUN — NOT A BENCHMARK RESULT.** Limited batches/epochs on real "
            "data, run only to verify the pipeline works end to end. These numbers are not "
            "meaningful and must not be published, cited, or compared against anything.",
            "",
        ]
    lines += [
        "masstrust-only baseline — no competitor reproduction in this report.",
        "",
        f"- model: {manifest.get('model_name', 'unknown')}",
        f"- dataset_version: {manifest.get('dataset_version', 'unknown')}",
        f"- candidate_pool: {manifest.get('candidate_pool', 'unknown')}",
        f"- seed: {manifest.get('seed', 'unknown')}",
        f"- run_kind: {run_kind}",
        f"- masstrust_commit: {manifest.get('masstrust_commit', 'unknown')}"
        f"{' (dirty working tree!)' if manifest.get('working_tree_dirty') else ''}",
        f"- top-1 accuracy (test, all queries): {acc:.4f}",
        "",
        "**Caveat:** the best checkpoint is selected by a *validation* metric, and the "
        "masstrust threshold is then calibrated on that same val fold — val confidence "
        "scores are therefore mildly optimistic relative to test. This is the standard "
        "protocol (matching upstream MassSpecGym), not a bug, but worth keeping in mind "
        "if test risk overshoots the target below.",
        "",
        "## Headline: Coverage@Risk-5% (score-gap)",
        "",
        f"- coverage achieved on test: {headline['test_coverage_achieved']}{headline_coverage_ci}",
        f"- risk achieved on test: {headline['test_risk_achieved']}{headline_risk_wilson}",
        f"- target risk exceeded: {headline.get('target_risk_exceeded')}",
        f"- accepted: {headline['test_accepted']}/{headline['test_total']}, "
        f"errors: {headline['test_errors']}",
    ]
    if headline.get("abstain_all"):
        lines.append(f"- **ABSTAIN-ALL:** {headline.get('abstain_reason')}")
    lines += [
        "",
        "## Leakage checks",
        "",
    ]
    if leakage:
        lines += [
            f"- query_id overlap: {leakage['query_id_overlap']} / {leakage['n_test_queries']} "
            f"test queries ({leakage['query_id_overlap_pct']:.1f}%) — "
            f"{'HARD FAILURE' if leakage['hard_failure'] else 'none'}",
            f"- candidate pool overlap: {leakage['candidate_pool_overlap']} unique structures "
            "(stats only, not leakage by itself)",
            f"- formula overlap: {leakage['formula_overlap']} unique formulas "
            "(stats only, not leakage by itself)",
            f"- target-molecule overlap: {leakage['target_inchikey_overlap']} exact / "
            f"{leakage['target_inchikey_skeleton_overlap']} by 2D skeleton "
            "(reported, not hard-failed — see README)",
        ]
    else:
        lines.append("- `masstrust validate-split` did not produce a report; see stderr.")
    lines += [
        "",
        "## Full comparison",
        "",
        "| method | target risk | AURC | E-AURC | AURC 95% CI | unscoreable | test coverage | test risk |",
        "|---|---|---|---|---|---|---|---|",
    ]
    for r in rows_out:
        lines.append(
            f"| {r['method']} | {r['target_risk']} | {r['aurc']} | {r['eaurc']} | "
            f"[{r['aurc_ci_lo']}, {r['aurc_ci_hi']}] | {r['unscoreable_rate']:.4f} | "
            f"{r['test_coverage_achieved']} | {r['test_risk_achieved']} |"
        )
    lines += [
        "",
        "## Runtime",
        "",
        f"- cumulative masstrust CLI wall clock: {total_elapsed:.3f}s",
        f"- peak RSS across masstrust CLI calls: {peak_rss} KB (Linux) / bytes (macOS)",
    ]
    report_md.write_text("\n".join(lines) + "\n")

    print(f"Wrote {report_csv} and {report_md}")


if __name__ == "__main__":
    main()
