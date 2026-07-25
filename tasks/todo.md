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

### v0.3.0 — Evaluation Quality & Practical Tooling
- [x] Bootstrap confidence intervals for AURC (`--bootstrap N` on `curve`; `aurc_from_obs`, `bootstrap_aurc_ci`)
- [x] Additional confidence scoring: `score-ratio`, `topk-gap`, `effective-k`, `candidate-count`
- [x] `masstrust compare` — compare multiple scoring methods in one run
- [x] `masstrust drift` — KS-based distribution shift detection (calibration vs new data)
- [x] `masstrust validate-split` — leakage guard (query_id/inchikey/formula overlap, exit 1)

### v0.4.0 — Benchmark harness (in progress)
- [x] `masstrust evaluate` — evaluate a fixed policy threshold on separate, labeled held-out data (val-calibrate / test-evaluate, no recalibration on eval data)
- [x] Fix `compute_eaurc` to return NaN (not a biased κ) when unscoreable queries prevent full coverage
- [x] `benchmarks/massspecgym/` pipeline scaffolded — `prepare_data.py`, `run_baseline.py`, `validate_predictions.py`, `generate_report.py`, fixtures, `smoke_test.py` (all pass against the tiny fixture)
- [x] Harden benchmark provenance before the real run: best-checkpoint predictions (was silently using final-epoch weights), target-molecule leakage check, environment lock + run metadata in `manifest.json`, Coverage@Risk CI/Wilson bound in `masstrust evaluate` — see `CHANGELOG.md`
- [x] Real-data preflight completed on MassSpecGym — full pipeline (download → train → best-checkpoint reload → CSV export → validate-split → report) verified end to end against the pinned dataset revision; found and fixed 5 real bugs along the way (`load_massspecgym()` API mismatch, `huggingface_hub` pin conflict, `setuptools`/`pkg_resources`, a confirmed upstream massspecgym `RetrievalDataset`/`FingerprintFFNRetrieval` bug worked around locally, `is_correct` CSV casing) — see commits `27fb73f`/`a1072f9`. Preflight numbers themselves are meaningless (2 batches/1 epoch) and are not recorded here.
- [ ] Official seed-0 benchmark run: 50 epochs (real GPU training; see `benchmarks/massspecgym/README.md`) — not started yet, review the preflight report/manifest first

### Future / Research
- [ ] Validate CRC calibration on public MS/MS benchmarks (MassSpecGym) — pending the real-dataset run above
- [ ] Grouped calibration: additional examples and docs
- [ ] Calibration drift detection: group distribution shift (adduct, ion_mode)
- [ ] Conformal risk control with non-binary loss (monotone loss formulation)
- [ ] Grouped calibration by compound class (requires chemical taxonomy lookup)
- [ ] Probability calibration (temperature scaling) so max-prob/margin/entropy/effective-k can be benchmarked on MassSpecGym too
- [ ] Competitor comparison (Selective-MSMS, ms-cp, COSMIC/SIRIUS) — deliberately deferred; see benchmark harness README for the column contract that will allow this without core changes

## Backlog / Low Priority
- [ ] `masstrust-plot` separate crate (as noted in AGENTS.md)
- [ ] PNG output for plot (additional plotters backend)
- [ ] Optional `chematic` integration (molecule normalization, feature flag)
