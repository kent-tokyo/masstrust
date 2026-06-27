# Changelog

All notable changes to this project will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

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
