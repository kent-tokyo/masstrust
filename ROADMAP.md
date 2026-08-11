# masstrust — Roadmap

Status snapshot as of this commit. See each benchmark's own `README.md`/`FEASIBILITY.md` for full
detail.

## Shipped

- **v0.1.0–v0.3.0**: core selective-prediction engine — confidence scoring (max-prob, score-gap,
  margin, entropy, score-ratio, topk-gap, effective-k, candidate-count), risk-coverage curves,
  AURC/E-AURC with bootstrap CI, empirical/binomial/experimental-CRC calibration, grouped
  calibration, drift detection, leakage guard (`validate-split`), policy export/apply/batch, CLI +
  Python bindings, crates.io + PyPI releases.
- **v0.4.0 (core additions)**: `masstrust evaluate` (fixed-policy held-out evaluation, no
  recalibration), Coverage@Risk CI / Wilson bound reporting.
- **risksieve integration** (PR #1): feature-gated `risksieve`-backed `certify-batch` —
  theorem-backed SCoRE-SDR batch selective-deployment certification, independent of the
  reusable-threshold `calibrate`/`apply` flow. See `docs/risksieve-integration.md`. (Note: this
  was previously tracked in project memory as "blocked pending a crates.io release" — that was
  stale; it shipped.)
- **`benchmarks/massspecgym/`**: real-data harness against the official MassSpecGym v1.5
  retrieval benchmark. Preflight (small-batch, pipeline-verification) run completed successfully,
  5 real bugs found and fixed along the way. **Official seed-0 50-epoch run is blocked**: on this
  machine's Apple Silicon (MPS backend), training throughput projects to ~1 month, almost
  certainly RDKit per-candidate fingerprinting in `RetrievalDataset.__getitem__` (CPU-bound
  regardless of accelerator), not the accelerator itself. No checkpoint was lost (stopped at 9%
  of epoch 0). **Next step before relaunching**: profile the actual per-batch bottleneck
  (data loading vs. forward pass vs. fingerprinting) and consider `num_workers` tuning,
  precomputed/cached candidate fingerprints, larger batch size, or real CUDA hardware.
- **`benchmarks/selective_msms_external/`**: external query-confidence benchmark comparing
  masstrust's legacy calibration methods against risksieve-backed SCoRE-SDR certification, on
  Selective-MSMS's published per-query confidence (not a candidate-ranking importer — the exact
  v1 candidate-pool artifact was confirmed not publicly retrievable; scope was narrowed and
  named accordingly). Complete, with a documented assumption-unverified caveat on whole-batch
  exchangeability.
- **`benchmarks/dna_adductomics/`** (this round): Phase A literature/data reconnaissance for
  cancer-relevant DNA-adductomics MS/MS selective-annotation, with colibactin as the requested
  first killer use case. **Both the colibactin-specific benchmark and a general-DNA-adductomics
  benchmark are NO-GO** against this project's own pre-registered minimum-n floor — see
  `benchmarks/dna_adductomics/FEASIBILITY.md`. A real-data **pipeline-verification preflight**
  (not a benchmark) was built and runs end to end: real reference-standard MS/MS spectra, real
  matchms external scoring, real masstrust `calibrate`/`evaluate`/`compare` output, all
  `run_kind=preflight`-stamped. See that directory's `README.md` and `report/REPORT.md`.

## In progress / blocked

- MassSpecGym official seed-0 benchmark run (throughput-blocked, see above).
- Colibactin-specific DNA-adduct annotation benchmark: **blocked on data availability, not on
  masstrust or on this project's effort.** No public repository (GNPS/MassIVE/MetaboLights/
  Metabolomics Workbench/PRIDE) hosts candidate-ranking-ready colibactin MS/MS data; landmark
  structure papers keep evidence in supplementary PDFs only. Unblocks with either (a) a wet-lab
  collaborator sharing processed peak lists/fragment tables in machine-readable, redistributable
  form, or (b) a future public deposit — re-run `benchmarks/dna_adductomics/FEASIBILITY.md` §0's
  minimum-n check against it before calling anything a benchmark.
- General cancer-relevant DNA-adductomics benchmark: gated (by the brief that opened this work,
  §19) on the colibactin benchmark existing first. It doesn't yet. The best real substitute found
  (nexs-metabolomics DNA adductomics database, CC-BY 4.0) has only 8 distinct experimental
  reference-standard compounds — below the ≥15–20-distinct-compound floor this project
  pre-registered before looking at the data (`FEASIBILITY.md` §0). Unblocks if that database (or
  another CC-licensed one) grows past that floor with genuinely distinct compounds — re-check
  before calling it a benchmark.

## Backlog / research

- Probability calibration (temperature scaling) — would let `max-prob`/`margin`/`entropy`/
  `effective-k` be benchmarked on MassSpecGym and any future external-score dataset that only
  ships a bare score today.
- Competitor comparison beyond Selective-MSMS (ms-cp, COSMIC/SIRIUS) — deliberately deferred; see
  `benchmarks/massspecgym/README.md`'s column-contract note for how this drops in without core
  changes.
- Conformal risk control with non-binary loss (monotone loss formulation).
- Grouped calibration by compound class (needs a chemical taxonomy lookup) and additional
  drift-detection coverage (adduct/ion-mode group shift).
- `masstrust-plot` as its own crate; PNG plot output; optional `chematic` feature-gated
  integration for molecule normalization/fingerprints (see `AGENTS.md`).
- If a colibactin or general-DNA-adductomics dataset ever clears its pre-registered minimum-n
  floor: ground-truth-tier risk-coverage comparison (ready to run — needs a query population
  spanning more than one tier, which no current dataset has), and only *then*, if a
  domain-specific auxiliary evidence feature (neutral-loss support, isotope consistency,
  diagnostic fragments) is shown to generalize across more than one dataset, consider a
  general-purpose (not colibactin-specific) "auxiliary confidence feature" API in
  `masstrust-core` — never a `ColibactinScore`/`DNAAdductScore` type, per `AGENTS.md`'s
  non-goals and the brief that opened this work.
