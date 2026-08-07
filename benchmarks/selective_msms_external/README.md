# Selective-MSMS external query-confidence benchmark

**This is an external query-confidence benchmark, not a Selective-MSMS competitor-parity
benchmark, and not the official MassSpecGym v1.5 benchmark** (`benchmarks/massspecgym/`). It
compares masstrust's legacy threshold-calibration methods against its risksieve-backed SCoRE-SDR
certification, on the same fixed **query-level** confidence score published by Selective-MSMS.

**What this benchmark demonstrates:**
- Selective-MSMS's published query-level confidence and top-1 correctness can be fed into
  masstrust's existing input contract.
- On that same confidence score, masstrust's legacy threshold methods and its risksieve-backed
  SCoRE-SDR certification can be compared directly.
- A post-hoc risk-control workflow reproduces cleanly against a real external model's output.

**What this benchmark does not demonstrate:**
- A candidate-ranking importer, or candidate-pool compatibility with masstrust's own MassSpecGym
  v1.5 harness.
- Candidate identity reconstruction — this benchmark never had, and does not claim to have,
  per-candidate rankings for this artifact (see "What this uses" below).
- Recomputation of masstrust's candidate-list-dependent scoring methods (`score-gap`, `margin`,
  `topk-gap`, …) on this artifact.
- Selective-MSMS competitor parity, or a same-split/same-candidate-pool comparison against
  Selective-MSMS's own results.

See `benchmarks/selective_msms/PLAN.md` for the feasibility assessment and provenance spike this
benchmark resumes from (Verdict B, "Status: paused — resume after risksieve backend integration").

## A pre-registered stop condition was hit, and overridden — flagged for review

One of this phase's explicit stop conditions was: if the candidate-pool's meaning cannot be
reconstructed, stop and report honestly rather than proceed. That condition triggered: the exact
v1 candidate-pool JSON Selective-MSMS used is not publicly retrievable (see below). Rather than
halting, this run found an alternative source (`query_scores.parquet`, still inside the same
Zenodo release) that supplies query-level confidence and correctness — without the candidate
pool — and validated it three independent ways against figures already recorded in `PLAN.md`
before using it (see "What this uses" below). No candidate identity was fabricated or guessed at
any point, and the benchmark's name and scope claims were narrowed accordingly (this revision) to
match exactly what was actually reconstructed: query-level confidence, not candidate rankings.

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
   artifact from public sources — this benchmark does not have, and does not claim to have,
   candidate-level rankings.
2. **Query-level confidence and correctness turned out to be enough for the comparison this phase
   actually asks for.** `data/results/numerical/query_scores.parquet` (32 MB, also inside
   `results.zip`, fetched via the same HTTP-range "remote zip" technique as the original spike —
   see `scripts/fetch_query_scores.py`) contains one row per (query, K) with a `confidence`
   column and a `hit` (0/1 correctness) column. Verified directly against Selective-MSMS's own
   source (`source.zip` on the same Zenodo record,
   `ms_uq/unc_measures/retrieval_unc.py::RetrievalUncertainty.compute`, the code path selected
   by this table's own `feature_convention == "manuscript"` column): `confidence` is
   `confidence_top1`, the top-1 probability from `softmax(ensemble_mean_scores / T_eval)` with
   `T_eval = 0.003` (also a column in this table) — a direct score transform of the model's own
   output, not a meta-model or a separately-trained calibrator. Filtering to
   `run_label == "mlp_mass"`, `split == "test"`, `K == 1` reproduces, independently, three
   figures already recorded in `PLAN.md` from the original spike's smaller provenance files:
   Hit@1 = 0.140636 (matches `metrics.csv`'s 0.1406), 2,998 unique test molecules (matches
   `dataset_audit.csv`), and a candidate-record count sum of 4,457,058 (matches
   `metadata.json`/`evaluation_matrix.tsv`). That three-way agreement is the falsifiable identity
   check the original plan called `scores.pt`-opening for — it passed, without needing to
   deserialize `scores.pt` at all. `scripts/validate_source_schema.py` enforces all of these
   checks (plus null/finiteness/uniqueness checks) as hard failures before any split or
   comparison runs.

**Consequence:** this benchmark's calibration/evaluation methods compare purely on this fixed
per-query `confidence` score and per-query top-1 correctness — see "Scope metadata" below for the
explicit machine-readable record of what is and isn't available. `scores.pt` was never
downloaded. No `torch.load` call of any kind was made in this phase.

## Scope metadata

Recorded in `split_manifest.json`'s `scope` block (and repeated here so it isn't easy to miss):

```json
{
  "source_representation": "query_scores.parquet",
  "source_granularity": "query_level",
  "candidate_ranking_available": false,
  "candidate_pool_artifact_used": false,
  "candidate_identity_reconstructed": false,
  "confidence_source": "confidence_top1"
}
```

The `candidate_id` values in the derived CSVs (`{query_id}_top1`) are **adapter scaffolding** to
satisfy masstrust's CSV contract (which requires a `candidate_id` column) — they are not a
reconstructed or real candidate identity, and no claim about candidate ranking should be read
from their presence.

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
- `split_manifest.json` — **committed, immutable once created** (the `assignments`,
  `split_construction`, `counts`, and `source_artifact` blocks specifically — the `benchmark`/
  `purpose`/`scope` text fields may be corrected for accuracy without touching the split itself,
  as happened in this revision). The pre-registered calibration/evaluation assignment. If you
  need a different split, that is a new benchmark run.
- `results/` — `comparison_raw.json` (committed) is the full structured output of all 15 runs.
  `results/runs/` (gitignored) holds each run's raw `masstrust`/`certify-batch` output files
  (policy JSON, evaluation report JSON, certificate JSON, accepted/abstained CSVs, report.md).
- `REPORT.md` — the human-readable summary (committed).

## Reproducing

```
python3 scripts/fetch_query_scores.py          # 32 MB, checksum-verified against MANIFEST.tsv
python3 scripts/build_split_manifest.py        # validates schema, fails loudly if manifest exists
python3 scripts/convert_to_masstrust_csv.py
cargo build --release -p masstrust-cli --features risksieve
python3 scripts/run_comparison.py
```

Schema validation alone, without the 32 MB external file: `python3 scripts/validate_source_schema.py --selftest`.

## Pre-registration

Fixed **before** any of the 5 methods were run on this data:

- **Split:** group split by `molecule_group_id` (confirmed identical to the query's 14-character
  2D-InChIKey block), so no target molecule's queries appear in both halves. Seed 42, groups
  randomly assigned, ~50/50 by group count. Written to `split_manifest.json` by
  `scripts/build_split_manifest.py`, which refuses to overwrite an existing manifest. Chosen
  specifically to prevent target-molecule leakage between calibration and evaluation — see
  "Known limitation" below for what this split does and does not establish.
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

Neither SCoRE-SDR construction is known to dominate the other in general (coupled uses
cross-test-point information the independent construction discards, but this does not imply
coupled selects at least as much on every dataset) — see `REPORT.md` for what was actually
observed on this fixture, stated as a fixture-specific result, not a general ranking.

## Known limitation: whole-batch exchangeability is not established

Query-level joint exchangeability is not established because multiple spectra are clustered
within target molecules — the same tension Selective-MSMS's own `run_manifest.json` flags for
repeated-molecule spectra. The molecule-grouped split prevents target leakage, but does not by
itself establish the whole-batch exchangeability that SCoRE-SDR's guarantee (and, informally, the
other methods' calibration-transfers-to-evaluation logic) relies on.

To be precise about what was and wasn't done: the group split was chosen specifically to prevent
target leakage, not to engineer any particular difficulty distribution; calibration/evaluation
group membership was assigned by an unweighted random shuffle (seed 42) over molecule groups, not
selected to create a deliberate difficulty shift. `risksieve`'s certificate objects were computed
correctly by `risksieve` from the inputs given to them — nothing here alleges a bug in
`risksieve`. What is not established is that this benchmark's own data-generating structure
(clustered, molecule-correlated spectra) satisfies the theorem's exchangeability hypothesis. This
applies identically to all five methods, so it does not favor any one of them — but it means this
should be read as an **assumption-unverified diagnostic benchmark**: none of the numbers in
`REPORT.md` should be read as a formally verified guarantee on this exact split. Stated here and
in `REPORT.md`, not discovered by a reader after the fact.

## Forbidden framings (do not use in this benchmark's outputs)

Per the instructions that opened this phase: "Selective-MSMSに勝った", "同一splitでの競合比較",
"MassSpecGym v1.5公式benchmark", "candidate-pool parityが成立した",
"Selective-MSMSのcalibration/evaluation splitを再現した", "candidate-ranking importer",
"candidate identity reconstruction", "independent construction is strictly less powerful by
construction" (or any general dominance claim between the two SCoRE-SDR constructions) — none of
these describe what this benchmark does.
