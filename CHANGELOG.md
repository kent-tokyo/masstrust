# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Added

- `masstrust certify-batch --loss-column <name>` (requires `--features risksieve`): certify a
  SCoRE-SDR batch against any `[0, 1]`-bounded precomputed loss (e.g. Tanimoto dissimilarity,
  scaffold mismatch) instead of only binary top-1 correctness. Required on every scoreable
  calibration query; genuinely optional on `--test` — an unlabeled test set still certifies
  successfully, it just can't produce a post-hoc realized-risk number. `certificate.json`/
  `report.md` gain `loss_kind`/`loss_label`/`loss_column`/`loss_domain` provenance fields.
  Resolving realized risk under a different loss than what was certified is a hard error
  (`MasstrustError::LossSourceMismatch`), not a silently mismatched number. No new
  `masstrust-core`/`masstrust-cli` dependency — the loss value itself is always caller-supplied
  data (via new `io::read_query_loss_column`, CSV and Parquet), never computed by masstrust. See
  `docs/graded-loss-integration.md`. `Candidate`'s public shape is unchanged; `certify_batch`/
  `resolve_realized_losses` keep their exact pre-existing signatures as compatibility wrappers
  around new `certify_batch_with_loss`/`resolve_realized_losses_with_loss`.
- `benchmarks/dna_adductomics/`: literature/data reconnaissance for cancer-relevant
  DNA-adductomics MS/MS selective-annotation feasibility (colibactin as the requested killer use
  case) — see `FEASIBILITY.md`. **Verdict: NO-GO as a benchmark**, both for colibactin
  specifically (no public candidate-ranking-ready data found) and for the best general
  substitute found (8 distinct experimental compounds, below this project's own pre-registered
  minimum-n floor). A real-data pipeline-verification preflight (explicitly not a benchmark —
  see `PREFLIGHT_REPORT.md`) demonstrates the adapter → schema → `validate-split` → `calibrate`
  → `evaluate` pipeline against real, CC-BY 4.0-licensed third-party MS/MS data, with zero
  `masstrust-core` changes needed to carry nine domain provenance columns.

### Fixed

- `masstrust certify-batch`/`calibrate`/`apply`/Python `save_policy`+`load_policy`: a threshold
  computed via arithmetic (e.g. a score-gap threshold from subtracting two scores) could come
  back from `policy.json` as a different `f64` than what was written and certified against
  (`serde_json`'s default float parser is not round-trip-exact — fixed by enabling its
  `float_roundtrip` feature workspace-wide). Impact on accept/reject decisions was negligible
  (~1e-17 relative), but `policy.json` is documented as a "reproducible decision" artifact, and a
  save/load round trip silently changing the threshold's exact bit pattern violated that. New
  regression test (`test_policy_json_roundtrip_is_bit_exact_for_arithmetic_thresholds`) using an
  arithmetic-derived threshold — the pre-existing round-trip test used a literal that happened not
  to expose this. The CI Python wheel smoke test now also exercises `apply_policy`/`save_policy`/
  `load_policy`/`aurc`/`eaurc` (previously untested — only `compute_curve`/`calibrate` were
  covered).
- `benchmarks/massspecgym/run_baseline.py`: the throughput bottleneck behind the ~1-month/50-epoch
  projection recorded during the official seed-0 run attempt is `RetrievalDataset.__getitem__`
  recomputing an RDKit InChIKey (label matching) and Morgan fingerprint for every candidate
  (up to 256/query) on every access, with no caching — confirmed by `cProfile` to be ~98% of
  `__getitem__`'s wall time on real data. Since the candidate pool per query never changes across
  a run and both transforms are pure functions of the input SMILES, this was pure redundant
  recomputation (once per repeat spectrum of the same molecule within an epoch, and again every
  epoch). New `_CachingTransform` (memoizes both by SMILES, `--fingerprint-cache-size` to bound
  memory) is a benchmark-harness-only change — no `massspecgym`/`masstrust-core`/`masstrust-cli`
  changes. Measured ~73x end-to-end speedup (data loading + real forward/backward/optimizer step)
  once the cache is warm, on a controlled sample — see `README.md`'s Status section. The real
  50-epoch run itself has not been relaunched with this fix.

---

## [0.2.0] — 2026-08-08

### Added

- Optional, feature-gated `risksieve` backend (`--features risksieve`): `masstrust
  certify-batch`, a theorem-backed batch selective-deployment certification workflow built on
  `risksieve` 0.2.0's SCoRE-SDR controller (Bai and Jin, 2026, arXiv:2603.24704). Independent
  of `calibrate`/`apply` — not a reusable threshold policy, no changes to `PolicyFile` or the
  existing `CalibrationMethod`s. See `docs/risksieve-integration.md` for the full design
  rationale (estimand, score orientation, unscoreable-query/exchangeability policy) and the
  README's new "Batch selective-deployment certification" section for usage. Disabled by
  default; the existing CLI, API, and policy JSON schema are unaffected when the feature is
  off.
- MassSpecGym benchmark harness provenance hardening (pre-real-run):
  - `run_baseline.py` now reloads the best (not final-epoch) Lightning checkpoint via
    `ckpt_path="best"` before exporting val/test predictions, so the recorded
    `checkpoint_sha256` always matches the weights that actually produced the CSVs.
  - Exported prediction CSVs gain a `target_inchikey` column (the query's ground-truth
    molecule); `manifest.json` gains `env_info` (torch/CUDA/cuDNN/GPU/RDKit versions,
    masstrust git commit), `best_epoch`/`best_val_metric`, and a `requirements.lock.txt` +
    its sha256, written automatically at the end of every real run.
  - `masstrust validate-split`: candidate-pool and formula overlap are now stats-only
    (previously hard failures); a new target-molecule (`target_inchikey`) overlap check —
    a stronger leakage signal than pool overlap — is reported as a loud warning, not
    hard-failed. Only `query_id` overlap (the same spectrum in both splits) remains a
    hard failure. Adds `--out <path>.json` for a machine-readable report.
  - `masstrust evaluate`: adds `--bootstrap N` for a 95% CI on coverage and risk, a
    one-sided Wilson upper bound on risk, `target_risk_exceeded`, and an explicit
    `abstain_reason` when a policy accepts nothing on the evaluation set.

---

## [0.1.0] — 2025-06-27

### Added

**masstrust-core**
- `Candidate`, `QueryRanking`, `AnnotationDecision`, `PolicyFile`, `RiskCoverageRow` types
- CSV input with header validation and helpful error messages
- Parquet input via `polars` (opt-in, `--features parquet`, auto-detected by `.parquet` extension)
- Confidence scoring: `max_prob`, `score_gap`, `margin`, `entropy`
- Risk-coverage curve (`compute_curve`) — one row per distinct confidence value
- AURC and E-AURC metrics
- Empirical threshold calibration
- Conservative binomial (Wilson score) threshold calibration
- Experimental CRC-style threshold calibration (`1/(n+1)` finite-sample correction)
- Grouped calibration (`calibration::calibrate_grouped`): a separate threshold per subgroup
  (e.g. adduct type, instrument), with per-group thresholds and a global fallback for unknown
  groups. `Candidate.group` field, populated via `io::read_group_column`.
- Policy JSON export / import / apply (reproducible decisions)

**masstrust-cli**
- `masstrust curve` — compute risk-coverage curve; `--verbose` table, `--plot` SVG, `--histogram` SVG
- `masstrust calibrate` — calibrate threshold; richer report with AURC, E-AURC, CRC correction;
  `--group-col <column>` for grouped calibration (per-group thresholds stored in `policy.json`
  under `group_col`/`group_thresholds`)
- `masstrust apply` — apply policy to unlabeled candidates; writes trusted + abstained CSV;
  automatically reads the group column specified in the policy
- `masstrust batch` — apply one policy to multiple input files
- Optional SVG output via `plotters` (`--features plot`)

**masstrust-py**
- Python bindings via pyo3 0.22 / maturin
- `compute_curve`, `calibrate`, `apply_policy`, `load_policy`, `save_policy`, `aurc`, `eaurc`

**CI**
- GitHub Actions: fmt, clippy (`-D warnings`), test, doc — Ubuntu + macOS matrix
- Python wheel build and smoke test via maturin
- Security audit via cargo-audit

**Examples**
- `examples/labeled_candidates.csv` — minimal 4-query fixture
- `examples/candidates.csv` — unlabeled fixture for `apply`
- `examples/massspecgym_candidates.csv` — 8-query fixture with SMILES / InChIKey
- `examples/labeled_candidates_grouped.csv` — 8-query fixture with 3 adduct types, for testing
  grouped calibration

[Unreleased]: https://github.com/kent-tokyo/masstrust/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/kent-tokyo/masstrust/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/kent-tokyo/masstrust/releases/tag/v0.1.0
