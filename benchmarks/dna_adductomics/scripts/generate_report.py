#!/usr/bin/env python3
"""Render report/run_summary.json + data/candidates.csv into REPORT.md.

Every section is banner-labeled preflight/small-n. This script refuses to compute or print a
single "headline" Coverage@Risk-5% number the way benchmarks/massspecgym/generate_report.py does
for its real benchmark runs -- see ../FEASIBILITY.md ss2.4 for why that framing does not apply
here.
"""
import argparse
import csv
import json
import os

HERE = os.path.dirname(os.path.abspath(__file__))

BANNER = """> **PREFLIGHT — NOT A BENCHMARK.** 8 distinct compounds is below this project's own
> pre-registered minimum-n floor (`FEASIBILITY.md` ss0: >=15-20 distinct compounds for a
> defensible Coverage@Risk headline). Every number below is real (real spectra, real matchms
> scoring, real masstrust CLI output) but is reported to prove the adapter -> schema ->
> calibrate -> evaluate pipeline shape executes on real data, not as a validated selective-
> annotation result. See `FEASIBILITY.md` ss2.4 and `README.md`.
"""


def load_candidates(path):
    with open(path, newline="") as f:
        return list(csv.DictReader(f))


def top1_summary(rows):
    by_q = {}
    for r in rows:
        by_q.setdefault(r["query_id"], []).append(r)
    lines = []
    n_correct = 0
    for q, rs in sorted(by_q.items()):
        rs.sort(key=lambda r: int(r["rank"]))
        top1 = rs[0]
        correct = top1["is_correct"] == "true"
        n_correct += correct
        lines.append({
            "query_id": q, "name": None, "n_candidates": len(rs),
            "top1_candidate": top1["candidate_id"], "top1_score": float(top1["score"]),
            "top1_correct": correct, "genotoxicant_class": top1["genotoxicant_class"],
        })
    return lines, n_correct, len(by_q)


def render_eval_row(entry):
    score, cal_method, target = entry["score"], entry["calibration_method"], entry["target_risk"]
    r = entry.get("eval_result")
    if not r:
        return f"| {score} | {cal_method} | {target} | (calibrate found no qualifying threshold) | - | - | - |"
    risk = "n/a (0 accepted)" if r["risk"] is None else f"{r['risk']:.3f}"
    if r["target_risk_exceeded"] is True:
        risk += " **exceeded**"
    elif r["target_risk_exceeded"] is False:
        risk += " ok"
    wilson_ub = "n/a" if r["risk_wilson_upper"] is None else f"{r['risk_wilson_upper']:.3f}"
    coverage_cell = f"{r['accepted']}/{r['total']} ({r['coverage']:.2f})"
    note = r["abstain_reason"] or ""
    return f"| {score} | {cal_method} | {target} | {coverage_cell} | {risk} | {wilson_ub} | {note} |"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data-dir", default=os.path.join(HERE, "..", "data"))
    ap.add_argument("--report-dir", default=os.path.join(HERE, "..", "report"))
    args = ap.parse_args()

    candidates = load_candidates(os.path.join(args.data_dir, "candidates.csv"))
    top1_rows, n_correct, n_queries = top1_summary(candidates)

    with open(os.path.join(args.report_dir, "run_summary.json")) as f:
        summary = json.load(f)

    out = [
        "# DNA-adductomics masstrust preflight report",
        "",
        BANNER,
        "## Dataset",
        "",
        "nexs-metabolomics DNA adductomics database (CC-BY 4.0) — see `../FEASIBILITY.md` §2.1. "
        f"{n_queries} real experimental query compounds, real CFM-ID-predicted candidate pools "
        "(mass-filtered ±0.5 Da — at this dataset's scale the window is not load-bearing: pool "
        "sizes are identical from ±0.1 Da through ±0.5 Da, so this is not a tuned parameter), "
        "matchms CosineGreedy scoring. Compound-disjoint calibration (4 compounds) / test "
        "(4 compounds) split, verified leak-free by `masstrust validate-split` "
        "(`../data/validate_split_report.json`).",
        "",
        "**Label-conflation check** (is `is_correct=false` ever really a stereoisomer/duplicate "
        "of the true compound rather than a genuinely different molecule?): for every query where "
        "the top-1 pick was wrong, its InChIKey's first-14-character 2D-skeleton block was "
        "compared against the true compound's — same check `benchmarks/massspecgym/` uses for "
        "leakage. **All 4 wrong top-1 picks have a different skeleton block** (verified directly, "
        "not assumed); none is a stereoisomer of the answer. The accuracy/risk numbers below are "
        "not a labeling artifact.",
        "",
        "## Baseline: accept-all top-1 accuracy (all 8 queries, descriptive only)",
        "",
        f"**{n_correct}/{n_queries} correct ({n_correct/n_queries:.0%})** — real matchms cosine "
        "top-1 pick vs. the true compound. This is not a headline number either: n=8.",
        "",
        "| query (InChIKey) | genotoxicant class | n candidates | top-1 correct? | top-1 score |",
        "|---|---|---|---|---|",
    ]
    for row in top1_rows:
        out.append(
            f"| {row['query_id']} | {row['genotoxicant_class']} | {row['n_candidates']} | "
            f"{'yes' if row['top1_correct'] else '**no**'} | {row['top1_score']:.3f} |"
        )

    out += [
        "",
        "## Baseline method comparison (`masstrust compare`, full 8-query set, exploratory)",
        "",
        "AURC/E-AURC with bootstrap CI (n=200 resamples) across masstrust's four score-only "
        "confidence methods (matchms cosine is not a calibrated probability — `max-prob`/"
        "`margin`/`entropy`/`effective-k` are not applicable here, same reasoning as "
        "`benchmarks/massspecgym/README.md`). Raw output: `runs/compare.csv`.",
        "",
    ]
    compare_path = os.path.join(args.report_dir, "runs", "compare.csv")
    with open(compare_path, newline="") as f:
        compare_rows = list(csv.DictReader(f))
    out.append("| method | threshold | accepted/total@0.10 | AURC | E-AURC | AURC 95% CI |")
    out.append("|---|---|---|---|---|---|")
    for r in compare_rows:
        acc = f"{r['accepted']}/{r['total']}" if r["accepted"] else "n/a"
        threshold = f"{float(r['threshold']):.3g}" if r["threshold"] else "n/a"
        aurc = float(r["aurc"]) if r["aurc"] else float("nan")
        eaurc = float(r["eaurc"]) if r["eaurc"] else float("nan")
        ci = f"[{float(r['aurc_ci_lo']):.3f}, {float(r['aurc_ci_hi']):.3f}]" if r["aurc_ci_lo"] else "n/a"
        out.append(f"| {r['method']} | {threshold} | {acc} | {aurc:.3f} | {eaurc:.3f} | {ci} |")

    out += [
        "",
        "## Calibrate (4 compounds) -> evaluate held-out (4 compounds), all score x method x target",
        "",
        "Every combination actually run — including outright failures — per the small-n honesty "
        "requirement (brief §12). `abstain_reason` is masstrust's own machine-generated "
        "explanation when a policy accepts nothing.",
        "",
        "| score | calibration method | target risk | accepted/total (coverage) | realized risk | "
        "Wilson UB | note |",
        "|---|---|---|---|---|---|---|",
    ]
    for entry in summary["calibrate_evaluate"]:
        out.append(render_eval_row(entry))

    out += [
        "",
        "## What this does and does not show",
        "",
        "- The adapter → schema → `masstrust validate-split` → `calibrate` → `evaluate` pipeline "
        "runs end to end against real spectra, real external (matchms) scores, and real masstrust "
        "CLI output — not a toy fixture.",
        "- At n=4/4, several (score, target) combinations abstain on everything "
        "(`abstain_all: true`) or land on 50%-coverage/50%-realized-risk with a [0.0, 1.0] "
        "bootstrap CI — both are the honest small-n result, not a bug.",
        "- No Coverage@Risk-5% (or 10%, or 20%) number here should be read as a validated claim "
        "about masstrust's performance on cancer-relevant DNA-adduct annotation. See "
        "`../FEASIBILITY.md` §0/§2.3 for the pre-registered floor this dataset does not meet.",
        "- Ground-truth-tier comparison (brief §15) is not implemented: all 8 queries share one "
        "tier (`reference_standard`). See `../FEASIBILITY.md` §2.3.",
        "",
    ]

    report_path = os.path.join(args.report_dir, "REPORT.md")
    with open(report_path, "w") as f:
        f.write("\n".join(out) + "\n")
    print(f"wrote {report_path}")


if __name__ == "__main__":
    main()
