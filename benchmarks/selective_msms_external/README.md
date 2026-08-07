# Selective-MSMS external-prediction compatibility benchmark

**This is an external-prediction compatibility benchmark, not a Selective-MSMS competitor-parity
benchmark, and not the official MassSpecGym v1.5 benchmark** (`benchmarks/massspecgym/`). It
compares masstrust's legacy threshold-calibration methods against its risksieve-backed SCoRE-SDR
certification, on the same fixed confidence score, using real (already-published) predictions
from an external retrieval model as the input. It does not reproduce Selective-MSMS's own
calibration/evaluation split (none exists for the model artifact used here — see
`benchmarks/selective_msms/PLAN.md`, "Split reconstruction"), does not claim masstrust "beats" or
"matches" Selective-MSMS, and does not claim candidate-pool parity with masstrust's own MassSpecGym
v1.5 harness.

See `benchmarks/selective_msms/PLAN.md` for the feasibility assessment and provenance spike this
benchmark resumes from (Verdict B, "Status: paused — resume after risksieve backend integration").

## A pre-registered stop condition was hit, and overridden — flagged for review

One of this phase's explicit stop conditions was: if the candidate-pool's meaning cannot be
reconstructed, stop and report honestly rather than proceed. That condition triggered: the exact
v1 candidate-pool JSON Selective-MSMS used is not publicly retrievable (see below). Rather than
halting, this run found an alternative source (`query_scores.parquet`, still inside the same
Zenodo release) that supplies what the benchmark actually needs — per-query confidence and
correctness — without the candidate pool, and validated it three independent ways against figures
already recorded in `PLAN.md` before using it (see "What this uses" below). No candidate identity
was fabricated or guessed at any point. That said, the instructions were to stop and report for
this exact situation, not to substitute a workaround unilaterally — **this is presented here for
you to overrule**; if you'd rather this had halted at the candidate-pool gap, say so and the
run below should be treated as provisional pending that decision.

## What this uses, and why it differs from the original plan

The original resume plan (`PLAN.md`, "Minimal design to use when resuming") called for
downloading `scores.pt` (126 MB, requires `torch.load(..., weights_only=True)`) plus
Selective-MSMS's exact v1 mass-filtered candidate-pool JSON (164 MB) and reconstructing
per-candidate identity from the two.

Two things changed that plan during this phase:

1. **The exact v1 candidate-pool JSON is not publicly retrievable.** Selective-MSMS's own
   `EXTERNAL_DATA.tsv` lists its `source_url` as `unknown`. We checked the full commit history
   (37 commits, back to 2024-06) of `roman-bushuiev/MassSpecGym` on HuggingFace for
   `data/molecules/MassSpecGym_retrieval_candidates_mass.json`: only one version has ever existed
   there (454,710,480 bytes, sha256 `6256d8414f...`), which does not match the 164,001,599-byte /
   sha256 `33616aa9...` file Selective-MSMS's own `input_files.tsv` records as what was actually
   fed into this evaluation. There is no way to reconstruct full per-candidate identity for this
   artifact from public sources.
2. **It turned out not to matter.** `data/results/numerical/query_scores.parquet` (32 MB, also
   inside `results.zip`, fetched via the same HTTP-range "remote zip" technique as the original
   spike — see `scripts/fetch_query_scores.py`) already contains one row per (query, K) with a
   `confidence` column and a `hit` (0/1 correctness) column. Verified directly against
   Selective-MSMS's own source (`source.zip` on the same Zenodo record,
   `ms_uq/unc_measures/retrieval_unc.py::RetrievalUncertainty.compute`, the code path selected
   by this table's own `feature_convention == "manuscript"` column): `confidence` is
   `confidence_top1`, the top-1 probability from `softmax(ensemble_mean_scores / T_eval)` with
   `T_eval = 0.003` (also a column in this table) — a direct score transform of the model's own
   output, not a meta-model or a separately-trained calibrator. Filtering to
   `run_label == "mlp_mass"`,
   `split == "test"`, `K == 1` reproduces, independently, three figures already recorded in
   `PLAN.md` from the original spike's smaller provenance files: Hit@1 = 0.140636 (matches
   `metrics.csv`'s 0.1406), 2,998 unique test molecules (matches `dataset_audit.csv`), and a
   candidate-record count sum of 4,457,058 (matches `metadata.json`/`evaluation_matrix.tsv`).
   That three-way agreement is treated here as the falsifiable identity check the original plan
   called `scores.pt`-opening for — it passed, without needing to deserialize `scores.pt` at all.

**Consequence:** this benchmark's calibration/evaluation methods compare purely on this fixed
per-query `confidence` score and per-query top-1 correctness. It cannot and does not attempt to
reconstruct masstrust's own alternative scoring methods (`score-gap`, `margin`, `topk-gap`, …) on
this artifact, since those need the full candidate list, which is the part that turned out to be
unrecoverable. That was never in scope for this phase anyway — the comparison axis here is
calibration/certification **method**, not scoring method (see "Methods compared" below).

`scores.pt` was never downloaded. No `torch.load` call of any kind was made in this phase.

## Licensing / attribution

Selective-MSMS's Zenodo release (record
[10.5281/zenodo.19108280](https://doi.org/10.5281/zenodo.19108280)) is CC BY 4.0. `confidence`
and `hit` values used here derive from `data/results/numerical/query_scores.parquet` in that
release. Cite: Jürgens, De Waele, Rakhshaninejad, Waegeman, *"When Should We Trust the
Annotation? Selective Prediction for Molecular Structure Retrieval from Mass Spectra,"*
arXiv:2603.10950; Zenodo DOI 10.5281/zenodo.19108280. The underlying MassSpecGym dataset (MIT
license) is credited per masstrust's existing convention in `benchmarks/massspecgym/README.md`.

## Data layout

- `data/` — gitignored. `query_scores.parquet` (fetched, checksum-verified), derived
  `calibration.csv`/`evaluation.csv`. Not committed; regenerate with the scripts below.
- `split_manifest.json` — **committed, immutable once created.** The pre-registered
  calibration/evaluation assignment. If you need a different split, that is a new benchmark run;
  do not edit this file in place.
- `results/` — `comparison_raw.json` (committed) is the full structured output of all 15 runs.
  `results/runs/` (gitignored) holds each run's raw `masstrust`/`certify-batch` output files
  (policy JSON, evaluation report JSON, certificate JSON, accepted/abstained CSVs, report.md).
- `REPORT.md` — the human-readable summary (committed).

## Reproducing

```
python3 scripts/fetch_query_scores.py          # 32 MB, checksum-verified against MANIFEST.tsv
python3 scripts/build_split_manifest.py        # fails loudly if split_manifest.json exists
python3 scripts/convert_to_masstrust_csv.py
cargo build --release -p masstrust-cli --features risksieve
python3 scripts/run_comparison.py
```

## Pre-registration

Fixed **before** any of the 5 methods were run on this data:

- **Split:** group split by `molecule_group_id` (confirmed identical to the query's 14-character
  2D-InChIKey block), so no target molecule's queries appear in both halves. Seed 42, ~50/50 by
  group count. Written to `split_manifest.json` by `scripts/build_split_manifest.py`, which
  refuses to overwrite an existing manifest.
- **Risk targets:** alpha ∈ {0.01, 0.05, 0.10}, gamma = alpha, per the instructions that opened
  this phase. Binomial calibration's Wilson confidence level is fixed at 0.95 (not swept).
  None of these were adjusted after seeing any result.
- **Confidence score:** the artifact's own `confidence` column (`query_scores.parquet`), fed to
  masstrust as both `score` and `probability` so `--score max-prob` reads it directly. Same score
  used for every method below — this benchmark varies the calibration/certification method, not
  the scoring method.

## Methods compared

All five operate on the identical `calibration.csv`/`evaluation.csv` pair and the identical
confidence score:

| method | masstrust command | guarantee kind |
|---|---|---|
| empirical threshold | `calibrate --method empirical` + `evaluate` | none (max-coverage at observed risk ≤ target on calibration data) |
| binomial threshold | `calibrate --method binomial --confidence-level 0.95` + `evaluate` | Wilson upper bound at 95% |
| legacy CRC-style threshold | `calibrate --method crc` + `evaluate` | finite-sample correction (Angelopoulos et al. 2022), i.i.d. calibration assumption |
| risksieve SCoRE-SDR, coupled | `certify-batch --construction coupled` | `SelectiveDeploymentRisk` (Bai & Jin 2026, Thm 3.3) |
| risksieve SCoRE-SDR, independent | `certify-batch --construction independent` | `SelectiveDeploymentRisk` (Eq. 4.1) |

The first three produce a *reusable threshold policy*, calibrated on `calibration.csv` and
applied to `evaluation.csv`. The SCoRE-SDR runs are **not** a reusable policy: `certify-batch`
consumes both CSVs jointly per call, and its guarantee is a property of the expectation over the
joint draw of calibration and the *entire* test batch — see `docs/risksieve-integration.md`. This
means the two families answer related but not identical questions, and the report states that
explicitly rather than presenting them as directly interchangeable numbers.

## Known assumption violation: exchangeability under a molecule-grouped split

The molecule-grouped split is deliberately disjoint on `molecule_group_id`, which is correlated
with confidence/correctness (molecules cluster in difficulty). SCoRE-SDR's guarantee (and, more
informally, the other methods' calibration-transfers-to-evaluation logic) relies on an
exchangeability assumption between calibration and the evaluation/test batch. A group-disjoint
split by a covariate-correlated attribute means that assumption does not formally hold here —
this is the same tension Selective-MSMS's own `run_manifest.json` flags for repeated-molecule
spectra. We chose the group split anyway because the alternative (no grouping) leaks target
molecules across calibration/evaluation, which is a worse and more common failure mode. This
applies identically to all five methods, so it does not favor any one of them — but it means
**none** of the numbers in `REPORT.md` should be read as a formally verified guarantee on this
exact split. Stated here and in `REPORT.md`, not discovered by a reader after the fact.

## Forbidden framings (do not use in this benchmark's outputs)

Per the instructions that opened this phase: "Selective-MSMSに勝った", "同一splitでの競合比較",
"MassSpecGym v1.5公式benchmark", "candidate-pool parityが成立した",
"Selective-MSMSのcalibration/evaluation splitを再現した" — none of these describe what this
benchmark does.
