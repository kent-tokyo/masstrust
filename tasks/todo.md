# masstrust — TODO

## Done

### v0.1.0 — MVP CLI + Library
- [x] Cargo workspace setup
- [x] `masstrust-core`: types, error, io (CSV/JSON)
- [x] Confidence scoring: max_prob, score_gap, margin, entropy
- [x] Risk-coverage curve (`compute_curve`)
- [x] Empirical threshold calibration
- [x] Conservative binomial (Wilson) threshold calibration
- [x] Policy JSON export / import / apply
- [x] CLI: `curve`, `calibrate`, `apply` commands
- [x] Unit tests (41), integration tests (4), CLI smoke tests (6)

### v0.2.0 — Evaluation & Reporting
- [x] AURC metric (`compute_aurc`)
- [x] E-AURC metric (`compute_eaurc`)
- [x] Richer `calibrate` CLI report (threshold, coverage, observed risk, AURC, E-AURC)
- [x] `curve` command prints AURC / E-AURC to stderr
- [x] SVG risk-coverage plot (optional `--features plot`, via `plotters`)
  - `--plot <path>` flag on `curve` and `calibrate` commands
  - Target error rate drawn as horizontal line on calibrate plot
- [x] Confidence histogram SVG (`--histogram <path>` on `curve`, requires `--features plot`)
- [x] MassSpecGym-compatible example data (`examples/massspecgym_candidates.csv`, 8 queries with SMILES/InChIKey)
- [x] `--verbose` flag on `curve` — prints per-row table to stdout

## Next

### v0.3.0 — Ecosystem Integration
- [x] Python bindings (`crates/masstrust-py`, pyo3 0.22, `maturin build --features extension-module`)
  - `compute_curve`, `calibrate`, `apply_policy`, `load_policy`, `save_policy`, `aurc`, `eaurc`
  - `pip install target/wheels/masstrust-*.whl`
- [x] Parquet input support (`--features parquet` on masstrust-core, polars 0.46, auto-detect by extension)
- [x] Batch processing (`masstrust batch file1.csv file2.csv --policy p.json --out-dir ./results/`)
- [ ] Optional `chematic` integration (molecule normalization, feature flag)

### Future / Research
- [ ] Conformal risk control
- [ ] Grouped calibration (by instrument / adduct / compound class)
- [ ] Per-dataset calibration reports
- [ ] Bootstrap confidence intervals for AURC
- [ ] Calibration drift detection
- [ ] E-AURC when unscoreable queries present (currently returns NaN)

## Backlog / Low Priority
- [x] `cargo doc` polish — `RUSTDOCFLAGS="-D warnings"` clean, 3 doctests added
- [x] GitHub Actions CI (`.github/workflows/ci.yml`)
  - `check` job: fmt, clippy, test, doc — ubuntu + macos matrix
  - `python` job: maturin build + Python smoke test
  - `audit` job: cargo-audit
- [ ] `masstrust-plot` separate crate (as noted in AGENTS.md)
- [ ] PNG output for plot (requires additional plotters backend)
