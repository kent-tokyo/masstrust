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
- [x] Unit tests (41+), integration tests, CLI smoke tests
- [x] Experimental CRC-style calibration (`--method crc`, `1/(n+1)` finite-sample correction)
- [x] Grouped calibration (`--group-col <col>`, per-group thresholds in policy JSON)
- [x] Python bindings via pyo3 + maturin (`compute_curve`, `calibrate`, `apply_policy`, `aurc`, `eaurc`)
- [x] Parquet input (`--features parquet`, auto-detected by extension)
- [x] Batch processing (`masstrust batch`)
- [x] Cargo metadata + dual MIT OR Apache-2.0 license
- [x] GitHub release v0.1.0 + crates.io publish (masstrust-core, masstrust-cli)

### v0.2.0 — Evaluation & Reporting
- [x] AURC metric (`compute_aurc`)
- [x] E-AURC metric (`compute_eaurc`)
- [x] Richer `calibrate` CLI report (threshold, coverage, observed risk, AURC, E-AURC)
- [x] `curve` command prints AURC / E-AURC to stderr
- [x] SVG risk-coverage plot (`--features plot`, `--plot <path>`)
- [x] Confidence histogram SVG (`--histogram <path>`)
- [x] MassSpecGym-compatible example data (`examples/massspecgym_candidates.csv`)
- [x] `--verbose` flag on `curve`

### Infrastructure
- [x] `cargo doc` polish — `RUSTDOCFLAGS="-D warnings"` clean, doctests added
- [x] GitHub Actions CI (fmt, clippy, test, doc, maturin wheel, cargo-audit)
- [x] README, LICENSE-MIT, LICENSE-APACHE, CHANGELOG, CONTRIBUTING

---

## Next

### Future / Research
- [ ] Validate CRC calibration on public MS/MS benchmarks (MassSpecGym)
- [ ] Grouped calibration: additional examples and docs
- [ ] Bootstrap confidence intervals for AURC
- [ ] Calibration drift detection
- [ ] E-AURC when unscoreable queries present (currently returns NaN)
- [ ] Conformal risk control with non-binary loss (monotone loss formulation)
- [ ] Grouped calibration by compound class (requires chemical taxonomy lookup)

## Backlog / Low Priority
- [ ] `masstrust-plot` separate crate (as noted in AGENTS.md)
- [ ] PNG output for plot (additional plotters backend)
- [ ] Optional `chematic` integration (molecule normalization, feature flag)
