# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

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

### Added

- Experimental CRC-style threshold calibration (`--method crc`): applies a `1/(n+1)`
  finite-sample correction to the empirical target, inspired by Angelopoulos et al. (2022).
  Assumes i.i.d. calibration data and binary 0/1 annotation loss.  Expressed as experimental;
  see `calibration::calibrate_crc` docs for assumptions and limitations.
- Grouped calibration (`--group-col <column>`): calibrates a separate threshold per subgroup
  (e.g. adduct type, instrument).  Per-group thresholds are stored in `policy.json` under
  `group_col` and `group_thresholds`; queries with an unknown group fall back to the global
  threshold.  `masstrust apply` automatically reads the group column specified in the policy.
- `examples/labeled_candidates_grouped.csv`: 8-query fixture with 3 adduct types for testing
  grouped calibration.
- `Candidate.group` field for group assignment (populated via `io::read_group_column`).

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
- Policy JSON export / import / apply (reproducible decisions)

**masstrust-cli**
- `masstrust curve` — compute risk-coverage curve; `--verbose` table, `--plot` SVG, `--histogram` SVG
- `masstrust calibrate` — calibrate threshold; richer report with AURC, E-AURC, CRC correction
- `masstrust apply` — apply policy to unlabeled candidates; writes trusted + abstained CSV
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

[Unreleased]: https://github.com/kent-tokyo/masstrust/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kent-tokyo/masstrust/releases/tag/v0.1.0
