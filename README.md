# masstrust

Calibrated trust and abstention for MS/MS molecular annotations.

[![CI](https://github.com/kent-tokyo/masstrust/actions/workflows/ci.yml/badge.svg)](https://github.com/kent-tokyo/masstrust/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

[日本語](README_ja.md) | [中文](README_zh.md)

---

## What masstrust does

`masstrust` is a Rust toolkit for **selective prediction** in MS/MS molecular annotation.

It takes candidate rankings from external annotation or retrieval tools and decides whether the top-ranked molecular annotation should be **trusted** or **abstained** under a user-specified error-rate target.

The core idea:

> Trust confident annotations. Abstain when uncertainty is too high.

## What masstrust does NOT do

- Does not perform MS/MS database search or spectral matching
- Does not generate molecular structures
- Does not perform retrosynthesis
- Does not replace SIRIUS, CSI:FingerID, MassSpecGym, or similar tools
- Does not guarantee clinical correctness

`masstrust` is a **post-hoc trust layer** for annotation pipelines. It sits after your annotation tool of choice and controls the risk of accepting incorrect annotations.

---

## Installation

### Rust CLI (from source)

```bash
git clone https://github.com/kent-tokyo/masstrust
cd masstrust
cargo install --path crates/masstrust-cli
```

### Python wheel (requires [maturin](https://maturin.rs))

```bash
maturin build --features extension-module
pip install target/wheels/masstrust-*.whl
```

---

## Quick start — CLI

**Step 1.** Compute the risk-coverage curve from labeled candidates:

```bash
masstrust curve examples/labeled_candidates.csv \
  --score score-gap \
  --out risk_coverage.csv \
  --verbose
```

**Step 2.** Calibrate a threshold at 5% error rate:

```bash
masstrust calibrate examples/labeled_candidates.csv \
  --score score-gap \
  --error-rate 0.05 \
  --method empirical \
  --out policy.json
```

**Step 3.** Apply the policy to new (unlabeled) candidates:

```bash
masstrust apply examples/candidates.csv \
  --policy policy.json \
  --out trusted_annotations.csv \
  --abstained abstained.csv
```

**Batch mode** — apply one policy to many files:

```bash
masstrust batch data/*.csv \
  --policy policy.json \
  --out-dir ./results/
```

**Compare multiple scoring methods** on one labeled dataset:

```bash
masstrust compare examples/labeled_candidates.csv \
  --scores score-gap,score-ratio,topk-gap,candidate-count \
  --error-rate 0.05 --method empirical \
  --bootstrap 1000 \
  --out compare.csv
```

**Evaluate a policy on separate held-out data** — calibrate on one set (e.g. validation), evaluate on another (e.g. test) without recalibrating:

```bash
masstrust calibrate val.csv --score score-gap --error-rate 0.05 --method empirical --out policy.json
masstrust evaluate test.csv --policy policy.json --out eval_report.json
```

**Detect confidence drift** between calibration and new data:

```bash
masstrust drift --calibration examples/labeled_candidates.csv --new examples/candidates.csv \
  --score score-gap --out drift_report.json
```

**Guard against calibration/test leakage** (`query_id`/`inchikey`/`formula` overlap):

```bash
masstrust validate-split --calibration val.csv --test test.csv
```

### Sample output

```
$ masstrust calibrate examples/massspecgym_candidates.csv \
    --score score-gap --error-rate 0.05 --method empirical --out policy.json

Calibration result (ScoreGap, empirical):
  target error rate: 0.0500
  global threshold:  0.120000
  coverage:          0.5000  (4/8 queries accepted, 50.0%)
  observed risk:     0.0000  (0/4 errors)
  AURC:              0.151488
  E-AURC:            -0.001938

$ masstrust apply examples/candidates.csv --policy policy.json \
    --out trusted.csv --abstained abstained.csv

Accepted: 1  Abstained: 1  (wrote trusted.csv and abstained.csv)
```

**SVG plots** (requires `--features plot`):

```bash
cargo build --features plot
masstrust curve examples/labeled_candidates.csv \
  --score score-gap --out risk.csv \
  --plot risk.svg --histogram hist.svg
```

---

## Quick start — Python

```python
import masstrust

# 1. Compute risk-coverage curve
curve = masstrust.compute_curve(
    "examples/labeled_candidates.csv",
    score="score-gap",
)
print(f"AURC: {masstrust.aurc(curve):.4f}")

# 2. Calibrate
policy = masstrust.calibrate(
    "examples/labeled_candidates.csv",
    score="score-gap",
    error_rate=0.05,
    method="empirical",
)
print(f"Threshold: {policy['threshold']:.4f}")

# 3. Apply to new data
decisions = masstrust.apply_policy("examples/candidates.csv", policy)
accepted  = [d for d in decisions if d["accepted"]]
print(f"Accepted: {len(accepted)}/{len(decisions)}")

# Save / load policy
masstrust.save_policy("policy.json", policy)
policy = masstrust.load_policy("policy.json")
```

---

## Input CSV format

```csv
query_id,candidate_id,rank,score,probability,is_correct
q1,cand_a,1,0.92,0.71,true
q1,cand_b,2,0.81,0.21,false
q2,cand_c,1,0.88,0.46,false
q2,cand_d,2,0.86,0.43,true
```

| Column | Required | Description |
|--------|----------|-------------|
| `query_id` | ✓ | Spectrum identifier |
| `candidate_id` | ✓ | Candidate structure identifier |
| `rank` | ✓ | Rank among candidates for this query (1 = best) |
| `score` | ✓ | Raw annotation score (higher is better) |
| `probability` | — | Posterior probability (required for `max-prob`, `margin`, `entropy`) |
| `is_correct` | — | Ground-truth label; required for `calibrate` and `curve` |

Parquet input is supported with `--features parquet` (auto-detected by `.parquet` extension).

Add `--bootstrap 1000` to `curve` or `compare` for a bootstrap 95% confidence interval on AURC.

---

## Confidence scoring methods

| Method | Formula | Requires |
|--------|---------|---------|
| `score-gap` | `score(rank-1) − score(rank-2)` | ≥2 candidates |
| `score-ratio` | `score(rank-1) / score(rank-2)` | ≥2 candidates, `score(rank-2) > 0` |
| `topk-gap` | `score(rank-1) − mean(score(rank-2..min(5,n)))` | ≥2 candidates |
| `candidate-count` | `1 / n_candidates` | none — always scoreable |
| `max-prob` | `probability(rank-1)` | `probability` column |
| `margin` | `probability(rank-1) − probability(rank-2)` | `probability`, ≥2 candidates |
| `entropy` | `1 − H_normalized` over all candidate probabilities | `probability` for all candidates |
| `effective-k` | `exp(−H)` over all candidate probabilities | `probability` for all candidates |

---

## Calibration methods

| Method | Behaviour | Notes |
|--------|-----------|-------|
| `empirical` | Maximum coverage where observed risk ≤ target | No statistical guarantee beyond validation set |
| `binomial` | Maximum coverage where one-sided Wilson upper bound ≤ target | Conservative; add `--confidence-level 0.95` |
| `crc` *(experimental)* | Empirical target tightened by `1/(n+1)` | CRC-style correction; see caveats below |

```bash
# Empirical
masstrust calibrate labeled.csv --score score-gap --error-rate 0.05 --method empirical --out policy.json

# Binomial (conservative)
masstrust calibrate labeled.csv --score score-gap --error-rate 0.05 --method binomial --confidence-level 0.95 --out policy.json

# CRC-style (experimental)
masstrust calibrate labeled.csv --score score-gap --error-rate 0.05 --method crc --out policy.json
```

---

## Batch selective-deployment certification (`risksieve`, optional)

`masstrust certify-batch` is a **feature-gated** (`--features risksieve`), independent
workflow, built on the [`risksieve`](https://crates.io/crates/risksieve) crate's SCoRE-SDR
controller (Bai and Jin, 2026, arXiv:2603.24704). It is not exposed at all in a default build.

**This is not a reusable threshold policy.** Unlike `calibrate`/`apply`, it computes a fresh
selection for one specific calibration set + test batch pair every time you run it — the
default construction's e-values depend on the whole test batch's composition, so the same
calibration data can select a different subset of a different test batch. See
[`docs/risksieve-integration.md`](docs/risksieve-integration.md) for why this is a separate
command rather than a new `CalibrationMethod`, and exactly what
`GuaranteeKind::SelectiveDeploymentRisk` does and does not claim.

```bash
masstrust certify-batch \
  --calibration val.csv \
  --test test.csv \
  --score score-gap \
  --alpha 0.05 \
  --gamma 0.05 \
  --construction coupled \
  --accepted accepted.csv \
  --abstained abstained.csv \
  --certificate certificate.json \
  --report report.md
```

- The risk guarantee is conditional on stated assumptions (exchangeability of calibration and
  the *entire* test batch jointly, a `[0, 1]`-bounded loss, and so on) — `certificate.json` and
  `report.md` both record them in full; read them before trusting a number.
- **Zero selections is a normal, valid certificate**, not an error — SCoRE-SDR's bound holds
  trivially on an empty selected set.
- If `test.csv` happens to carry `is_correct` labels, the report also prints a **realized
  selective risk** — a post-hoc descriptive statistic computed from this one batch's actual
  outcomes. It is kept clearly separate from the theorem-backed certificate above it and never
  described as validating (or invalidating) that certificate.

### Graded chemical loss (`--loss-column`)

By default the certified/realized loss is binary top-1 correctness (`is_correct`: `0.0` if
correct, `1.0` otherwise). `--loss-column <name>` certifies against any `[0, 1]`-bounded
precomputed loss instead — e.g. Tanimoto dissimilarity or scaffold mismatch, computed upstream
(masstrust never computes chemistry itself; see [`docs/graded-loss-integration.md`](docs/graded-loss-integration.md)):

```bash
masstrust certify-batch \
  --calibration val.csv \
  --test test.csv \
  --score score-gap \
  --alpha 0.05 \
  --gamma 0.05 \
  --loss-column tanimoto_loss \
  --accepted accepted.csv \
  --abstained abstained.csv \
  --certificate certificate.json \
  --report report.md
```

- Required on every scoreable **calibration** query (hard error if the column is absent, or a
  value is missing/malformed/outside `[0, 1]`/non-finite). Genuinely optional on `--test` — an
  unlabeled test set (no such column at all) still certifies successfully; it just can't
  produce a realized-risk number.
- `certificate.json`/`report.md` record `loss_kind`/`loss_label`/`loss_column`/`loss_domain` so
  a `--loss-column` certificate can never be misread as certifying exact-match risk.
- Realized risk is always resolved against the *same* loss the certificate itself used — asking
  for it under a different `--loss-column` (or under binary correctness, for a graded-loss
  certificate) is a hard error, not a silently mismatched number.

`masstrust`'s existing calibration methods (`empirical`/`binomial`/`crc`) are untouched by this
feature — see the scientific caveats below, which apply to `certify-batch` too.

---

## Understanding risk and coverage

The **risk-coverage curve** summarises the accept/abstain tradeoff:

- **Coverage** = fraction of queries where a prediction is accepted
- **Risk** = fraction of accepted predictions that are incorrect

At any threshold, higher coverage comes at higher risk. `masstrust` finds the threshold that maximises coverage while keeping risk within the specified target.

```
risk
1.0 ┤                          ╭──
    │                     ╭────╯
    │                ╭────╯
0.05┤ ─ ─ ─ ─ ─ ─ ─╱── target
    │           ╭───╯
0.0 ┤───────────╯
    └──────────────────────────── coverage
    0                             1.0
```

The threshold is chosen as the rightmost point on the curve that stays at or below the target risk line.

---

## Policy JSON

A calibrated policy is saved as a reproducible JSON file:

```json
{
  "version": "0.1.0",
  "scoring_method": "score_gap",
  "threshold": 0.18,
  "target_error_rate": 0.05,
  "calibration_method": "empirical",
  "confidence_level": null,
  "created_by": "masstrust"
}
```

---

## Benchmarking on MassSpecGym

`benchmarks/massspecgym/` is a self-contained pipeline (kept out of the Rust workspace — it depends on `massspecgym`/torch/rdkit) that trains the official Fingerprint FFN retrieval baseline, exports its predictions in masstrust's schema, and reports masstrust's own scoring methods' AURC/E-AURC/coverage-at-risk on real MassSpecGym data. This establishes a real, reproducible benchmark before any competitor comparison — none is claimed yet.

The harness has been validated end to end against real data via a small-scale preflight run (download → train → checkpoint → predictions → report), which found and fixed several real integration bugs along the way. The full benchmark run is not complete yet — no results are published. See `benchmarks/massspecgym/README.md` for the full protocol and current status.

---

## Scientific caveats

- `masstrust` **controls observed or bounded risk under the chosen calibration procedure on the provided validation data**. It does not guarantee correctness on out-of-distribution spectra.
- Risk control is calibrated on the validation set provided. If the test distribution differs (different instrument, adduct type, compound class), guarantees may not transfer.
- The experimental `crc` method assumes i.i.d. calibration data and binary 0/1 annotation loss. Verify these assumptions hold for your dataset before relying on the stated guarantee.
- Small calibration sets (< ~20 queries) may produce very conservative thresholds or no valid threshold at all (reported as `threshold = +inf`, i.e. abstain on everything).
- `masstrust` does not claim clinical validity or regulatory compliance.

---

## References

- Angelopoulos, A. N., Bates, S., Fisch, A., Lei, L., & Schuster, R. (2022). **Conformal Risk Control.** *arXiv:2208.02814.* — basis for the `crc` calibration method.
- Geifman, Y., & El-Yaniv, R. (2017). **Selective classification for deep neural networks.** *NeurIPS.* — foundational selective prediction framework.

---

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
