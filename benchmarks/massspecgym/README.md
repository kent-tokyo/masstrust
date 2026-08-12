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

## Status

- **Harness**: hardened and passing `smoke_test.py` against the fixture (see
  `CHANGELOG.md`).
- **Preflight** (real data, limited batches — see below): passed end to end.
  Found and fixed 5 real bugs along the way — see
  [Known issues found during the real-data preflight](#known-issues-found-during-the-real-data-preflight).
- **Official seed-0 run**: attempted, not yet completed. The first attempt hit
  a real `DataLoader`-worker crash (fixed — see below). After that fix, the
  run itself progressed but at a rate that would take on the order of a
  month for 50 epochs on the machine tested so far (Apple Silicon, MPS
  backend). Stopped before any checkpoint was written; no benchmark numbers
  exist yet.
- **Throughput bottleneck profiled and identified**: `cProfile` against the
  real dataset confirms the ~1-month projection above is almost entirely
  `RetrievalDataset.__getitem__` re-computing an RDKit InChIKey (for label
  matching) and Morgan fingerprint for every candidate (up to 256/query) on
  every access, with zero caching upstream — ~98% of `__getitem__`'s wall
  time in a real, unshuffled 192-item sample, and the InChIKey label pass is
  actually larger than the fingerprint pass (11.5s vs 6.2s). Since the
  candidate pool per query is fixed for the whole run and both transforms are
  pure functions of the input SMILES, this is purely redundant recomputation
  — the same query's candidates get re-transformed once per repeat spectrum
  of that molecule within an epoch, and again on every one of 50 epochs.
  `run_baseline.py`'s new `_CachingTransform` (module-level, so it survives
  massspecgym's `persistent_workers=True` default) memoizes both by SMILES
  string, no upstream/massspecgym changes needed. Measured effect on a
  controlled, unshuffled, real (data loading + model forward/backward/
  optimizer step) 3-batch sample: **12.48s/batch (no cache) → 1.66s/batch
  (cache, first pass) → 0.17s/batch (cache, same items re-fetched — the
  realistic steady state for epoch 2 onward)**, a ~73x end-to-end speedup
  once the cache is warm. This is a throughput fix, not a new benchmark run —
  see "Not yet done" below for what relaunching the real 50-epoch run still
  needs.
- **No benchmark numbers have been published or recorded anywhere** — only
  preflight runs (explicitly non-representative, small-batch) have completed.

### Not yet done

- The real 50-epoch seed-0 run has **not** been relaunched with this fix —
  profiling and the fix above were deliberately scoped to throughput
  investigation only, not a new training run. The ~73x figure is from a
  3-batch controlled sample, not a full-epoch measurement; a full run could
  behave somewhat differently (fingerprint-cache memory pressure at full
  scale, `num_workers>0` behavior, val/test-fold cache misses on first
  evaluation, GPU/MPS characteristics over a period of hours rather than
  seconds).
- `--fingerprint-cache-size` (default 200,000 entries, ~3.2GB at
  `--fp-size 4096`) is an untuned default — worth revisiting once a real run
  is attempted.

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
| `run_kind` | `preflight` (limited batches/epochs, pipeline smoke-check only) or `benchmark` (full run) — `generate_report.py` refuses to present `preflight` numbers as a benchmark result |
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
| `masstrust_commit` | `git rev-parse HEAD` in this repo — which exact code produced this run |
| `working_tree_dirty` | `true` if `git status --porcelain` was non-empty at run time — `masstrust_commit` alone doesn't cover uncommitted changes |
| `run_kind` | `preflight` or `benchmark` — see the `run_kind` row in `## Output schema` above |
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
#    (real training run, --num-workers > 0 recommended — see the
#    performance note below):
python run_baseline.py --out-dir ./data --seed 0 --num-workers 4

# 3. Validate the exported predictions (schema, leakage, quality gate):
python validate_predictions.py --val ./data/val_predictions.csv --test ./data/test_predictions.csv

# 4. Generate the benchmark report:
python generate_report.py --val ./data/val_predictions.csv --test ./data/test_predictions.csv \
    --out-dir ./report --bootstrap 1000
```

Step 0 is the only part of this that's fast and dependency-light; it's what
should run in CI-like contexts. Steps 1–2 need real compute and network
access — see `## Status` above and `tasks/todo.md` at the repo root for where
this currently stands.

**Performance note:** upstream's reference setup assumes a real CUDA GPU.
On hardware without CUDA (tested: Apple Silicon via the MPS backend),
`accelerator="gpu"` still resolves and runs, but training throughput can be
dramatically worse than expected — likely because `RetrievalDataset.__getitem__`
computes an RDKit fingerprint per candidate per query (up to 256
candidates/query) on CPU, regardless of which accelerator the model itself
runs on. Before committing to a full 50-epoch run on non-CUDA hardware,
time a handful of batches first (see Preflight below) and extrapolate; if
it's not GPU-cluster-scale hardware, expect this to matter.

### Preflight: real data, limited batches

Before spending a full 50-epoch GPU run, run the same steps 1–4 above against
real data but with `--run-kind preflight` and small `--limit-*-batches`, to
confirm the download, training loop, best-checkpoint reload, CSV export, and
report generation all actually work end to end — without producing a number
anyone could mistake for a result. `generate_report.py` reads `run_kind` from
the manifest and prints a loud non-benchmark banner at the top of `report.md`
when it's `preflight`. **This has been run successfully** — see `## Status`
above and the fixes it surfaced, listed below.

```bash
python prepare_data.py --out-dir ./data/preflight

python run_baseline.py \
    --out-dir ./data/preflight \
    --seed 0 \
    --max-epochs 1 \
    --accelerator cpu \
    --devices 1 \
    --num-workers 2 \
    --limit-train-batches 2 \
    --limit-val-batches 2 \
    --limit-test-batches 2 \
    --run-kind preflight

python validate_predictions.py \
    --val ./data/preflight/val_predictions.csv --test ./data/preflight/test_predictions.csv

python generate_report.py \
    --val ./data/preflight/val_predictions.csv --test ./data/preflight/test_predictions.csv \
    --manifest ./data/preflight/manifest.json --out-dir ./report/preflight --bootstrap 100
```

Deliberately not `--fast-dev-run`: Lightning's fast-dev-run mode can disable
checkpointing, which would skip exactly the best-checkpoint-reload path this
harness most needs to exercise. `--accelerator cpu` only if GPU isn't
available for the preflight; switch to `gpu` if a dependency or API
incompatibility is hard to diagnose on CPU alone.

Preflight passes when: data downloads at the pinned revision; training runs,
checkpoints, and reloads the best checkpoint; both CSVs are written with
`checkpoint_sha256`/`dataset_version`/`seed`/`run_kind` populated;
`validate-split` finds no query_id overlap and reports target-molecule stats;
every query has a true candidate; no non-finite scores or duplicate ranks;
`calibrate` → `evaluate` completes; and `report.csv`/`report.md`/policy and
evaluation JSON all get written. The accuracy numbers themselves are
meaningless (a handful of batches) and must not be published or recorded
anywhere as a result.

## Known issues found during the real-data preflight

None of this was catchable from the fixture — `smoke_test.py` never installs
massspecgym or touches real data, by design (fast, dependency-light, CI-safe).
These only surfaced by actually running the pipeline end to end. Listed here
so they're discoverable rather than buried in commit history; all are fixed
in the current code.

- **`massspecgym.utils.load_massspecgym()` takes no path argument** and
  always downloads its own unpinned copy of the *older* `MassSpecGym.tsv`
  internally — not the pinned `MassSpecGym1.5.tsv` this harness downloads and
  hashes. `prepare_data.py`/`run_baseline.py` read the pinned file directly
  with pandas instead.
- **`requirements.txt` had an unresolvable pin conflict**: a top-level
  `huggingface_hub==0.26.2` pin conflicted with `massspecgym==1.3.1`'s own
  hard pin of `huggingface-hub==0.23.2`. Removed the redundant top-level pin.
- **`pytorch-lightning==2.2.5`'s `lightning_fabric` needs `pkg_resources`**,
  which `setuptools>=81` removed. `requirements.txt` now pins `setuptools<81`.
- **Confirmed upstream bug in `massspecgym==1.3.1`** (also present on the
  unreleased `main` branch, checked against both): `RetrievalDataset.__getitem__`
  deletes `item["candidates"]` after using it, but `FingerprintFFNRetrieval.step()`
  (used for every train/val/test step) unconditionally reads
  `batch["candidates"]` — guaranteed `KeyError` on the very first step, using
  massspecgym's own reference wiring exactly as documented. Worked around in
  `run_baseline.py` with a small, documented subclass
  (`_RetrievalDatasetWithCandidates`) that restores the alias to the
  already-computed fingerprint tensor.
- **`is_correct` CSV casing**: pandas writes Python bools as `True`/`False`;
  masstrust-core's Rust CSV reader only accepts lowercase `true`/`false`.
  Normalized before writing.
- **`DataLoader(num_workers>0)` crashed immediately**: worker subprocesses
  need to serialize the dataset, and `_RetrievalDatasetWithCandidates` was
  originally a class defined inside `main()` — Python can't serialize a
  function-local class. `--num-workers 0`, used throughout the preflight
  above, never spawns workers and so never exercised this path; it only
  surfaced once a real run used `--num-workers > 0` for reasonable
  throughput. Fixed by moving the class (and the massspecgym imports it
  depends on) to module level.

## Explicitly out of scope for this round

Competitor reproduction, probability calibration/temperature scaling,
worst-group risk, multi-seed risk-violation rate, structure-disjoint/OOD
scenario comparison, non-binary/approximate correctness. See the repo's
planning notes for the reasoning and the intended follow-on order.
