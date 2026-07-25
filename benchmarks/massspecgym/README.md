# masstrust MassSpecGym benchmark harness

Runs masstrust's own confidence-scoring, calibration, and risk-coverage
engine end to end against real MassSpecGym retrieval predictions.

**Scope of this round:** masstrust's own numbers only. No Selective-MSMS,
ms-cp, or COSMIC/SIRIUS reproduction — see the repo's planning notes for why
and what comes next. This establishes the harness and a schema stable enough
that a stronger model's (or, later, a competitor's) prediction dump drops in
without touching `masstrust-core`.

This pipeline is intentionally kept out of the Rust workspace: it depends on
`massspecgym`, which pulls in torch/rdkit/torch_geometric/pytorch-lightning.
None of that touches any Rust crate.

## Protocol

1. **Data**: the official MassSpecGym retrieval dataset and the official
   "standard" (mass-filtered, ≤256 candidates/query) candidate pool, pinned
   to a specific HuggingFace revision — see `config.py`. Never a custom
   split: the official `fold` column (`train`/`val`/`test`) is used as-is.
2. **Model**: Fingerprint FFN, the simplest official retrieval baseline with
   a real per-candidate score (simpler than DeepSets/DeepSets+Fourier/MIST).
   **No pretrained checkpoint is published upstream** (checked: GitHub
   releases carry no binary assets) — `run_baseline.py` trains it from
   scratch, mirroring `scripts/run.py --model=fingerprint_ffn` in
   [pluskal-lab/MassSpecGym](https://github.com/pluskal-lab/MassSpecGym)
   exactly. This is a real GPU training run (default: 50 epochs, matching
   upstream) — it is not fast, and is not part of any test suite.
3. **Correctness**: `is_correct` is computed with massspecgym's own
   `MolToInChIKey` transform (candidate InChIKey == true-molecule InChIKey),
   exactly matching `RetrievalDataset`'s own labeling convention — not a
   custom identity rule.
4. **Calibrate on val, evaluate on test, never both on the same queries**:
   `masstrust calibrate` runs only on the validation dump; the resulting
   policy is applied to the test dump via `masstrust evaluate` (a new
   subcommand — see `crates/masstrust-cli/src/commands/evaluate.rs`), which
   reports the coverage/risk actually achieved without recalibrating.
   `evaluate --bootstrap N` adds a 95% CI on both coverage and risk, plus a
   one-sided Wilson upper bound on risk (at the policy's own
   `confidence_level` if it was calibrated with `--method binomial`, else
   95%). **Caveat:** the checkpoint used for both dumps is selected by a
   *validation* metric (see below), and the masstrust threshold is then
   calibrated on that same val fold — val confidence scores are therefore
   mildly optimistic relative to test. This is the standard protocol
   (matching what upstream MassSpecGym itself does), not a bug, but worth
   knowing if test risk overshoots the target.
5. `probability` is left out of the exported CSVs: these are raw model
   scores, not calibrated probabilities. Consequently only masstrust's four
   score-only methods are compared: `score-gap`, `score-ratio`, `topk-gap`,
   `candidate-count`. `max-prob`/`margin`/`entropy`/`effective-k` need a
   genuinely calibrated probability and are excluded this round, not by
   oversight — temperature scaling is future work.
6. **Coverage@Risk-5% on `score-gap`** is the pre-registered headline number.
   Everything else in the report is secondary.

## Output schema

Both `val_predictions.csv` and `test_predictions.csv` use masstrust's
standard candidate columns plus provenance columns that masstrust-core reads
right past (verified: `io::read_candidates` reads named columns only, extra
columns pass through untouched — no core schema change needed):

| column | meaning |
|---|---|
| `query_id`, `candidate_id`, `rank`, `score`, `is_correct` | masstrust's standard candidate schema |
| `split` | `val` or `test` |
| `model_name` | `fingerprint_ffn` |
| `checkpoint_sha256` | sha256 of the trained Lightning checkpoint |
| `dataset_version` | `MassSpecGym1.5` |
| `candidate_pool` | `MassSpecGym1.5_retrieval_candidates_mass.json` |
| `seed` | training seed |
| `target_inchikey` | InChIKey of the query's ground-truth molecule (distinct from `candidate_id`, this row's own candidate) — used by `masstrust validate-split` to detect answer-molecule leakage between val and test |

`checkpoint_sha256` is guaranteed to match the weights that actually produced
the row: `run_baseline.py` reloads the best (not final-epoch) Lightning
checkpoint via `ckpt_path="best"` before running either the test or val
dataloader, and hashes that same checkpoint file for the manifest.

Importing a different model's predictions later (a stronger public model, or
a future competitor comparison) means producing a CSV with these same
columns and a different `model_name`/`checkpoint_sha256` — nothing else
changes.

## Leakage checks

`masstrust validate-split` (run by `validate_predictions.py`, and again by
`generate_report.py` to capture stats for the report) distinguishes leakage
severity by what actually overlapped:

| overlap | meaning | severity |
|---|---|---|
| `query_id` | the same spectrum appears in both splits | **hard failure** (exit 1) — unambiguous |
| `target_inchikey` (full key, and 2D skeleton) | a val query's correct answer is also a test query's correct answer | reported loudly (`WARNING: ANSWER LEAKAGE`), not hard-failed — MassSpecGym's split guarantee for target-molecule disjointness isn't verified without the real dataset; promote to hard failure once Phase A shows the real overlap count |
| candidate pool (`inchikey`, falling back to `candidate_id`) | the same candidate structure recurs in both splits' pools | stats only — independently-sampled queries commonly share pool molecules |
| `formula` | the same molecular formula recurs in both splits | stats only — very common for unrelated molecules |

Pass `--out <path>.json` to `validate-split` for a machine-readable report of
all four counts (this is what `generate_report.py` reads into the report's
"Leakage checks" section).

## Manifest fields

`manifest.json` accumulates fields across `prepare_data.py` and
`run_baseline.py`. Beyond dataset/checkpoint identity, it records enough to
regenerate the same run:

| field | source |
|---|---|
| `env_info.torch_version`, `.cuda_version`, `.cudnn_version`, `.gpu_name`, `.rdkit_version` | best-effort, from the training environment |
| `env_info.masstrust_commit` | `git rev-parse HEAD` in this repo |
| `requirements_lock_sha256` | sha256 of `requirements.lock.txt`, a full `pip freeze` written alongside the run |
| `best_epoch`, `best_val_metric`, `best_val_metric_name` | from the `ModelCheckpoint` callback that selected the checkpoint actually used |

Each of these is best-effort and independently wrapped — a missing GPU,
RDKit, or git binary logs a warning to stderr but never discards an
otherwise-complete training run.

## Running it

```bash
pip install -r requirements.txt

# 0. Smoke-test the pipeline itself first (seconds, no GPU, no download):
python smoke_test.py

# 1. Pin and download the official dataset + candidate pool:
python prepare_data.py --out-dir ./data

# 2. Train the baseline and export val/test prediction dumps
#    (real GPU training run — see the warning above):
python run_baseline.py --out-dir ./data --seed 0

# 3. Validate the exported predictions (schema, leakage, quality gate):
python validate_predictions.py --val ./data/val_predictions.csv --test ./data/test_predictions.csv

# 4. Generate the benchmark report:
python generate_report.py --val ./data/val_predictions.csv --test ./data/test_predictions.csv \
    --out-dir ./report --bootstrap 1000
```

Step 0 is the only part of this that's fast and dependency-light; it's what
should run in CI-like contexts. Steps 1–2 need real compute and network
access and are not run automatically — see `tasks/todo.md` at the repo root
for status.

## Explicitly out of scope for this round

Competitor reproduction, probability calibration/temperature scaling,
worst-group risk, multi-seed risk-violation rate, structure-disjoint/OOD
scenario comparison, non-binary/approximate correctness. See the repo's
planning notes for the reasoning and the intended follow-on order.
