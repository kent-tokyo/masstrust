# masstrust — Roadmap

Status snapshot as of this commit. See each benchmark's own `README.md`/`FEASIBILITY.md` for full
detail.

## Released (crates.io + PyPI: `masstrust-core`, `masstrust-cli`, `masstrust-py`)

Verified directly against `Cargo.toml`'s `workspace.package.version`, `git tag`, `gh release
list`, and the crates.io/PyPI APIs — the only two published versions are:

- **v0.1.0** (2025-06-27): initial CLI + library — confidence scoring (max-prob, score-gap,
  margin, entropy), risk-coverage curves, AURC/E-AURC, empirical/binomial/experimental-CRC
  calibration, grouped calibration, policy export/apply/batch, Python bindings.
- **v0.2.0** (2026-08-07/08, including two follow-up packaging fixes): everything currently on
  `main`'s core crates — additional scoring methods (score-ratio, topk-gap, effective-k,
  candidate-count), `compare`/`drift`/`validate-split`/`evaluate` CLI commands, bootstrap CI on
  `curve`/`evaluate`, and the feature-gated `risksieve`-backed `certify-batch` (theorem-backed
  SCoRE-SDR batch selective-deployment certification, independent of the reusable-threshold
  `calibrate`/`apply` flow — see `docs/risksieve-integration.md`). (Note: `risksieve` integration
  was previously tracked in project memory as "blocked pending a crates.io release" — that was
  stale; it shipped in v0.2.0.) `Cargo.toml`'s workspace version is `0.2.0`; there is no
  released, tagged, or published v0.3.0/v0.4.0 — those were internal working labels in local
  task-tracking notes, not separate releases, and are not used here.

**On `main`, unreleased beyond v0.2.0**: graded chemical loss for `certify-batch`
(`--loss-column`) — certify a SCoRE-SDR batch against any `[0,1]`-bounded precomputed loss
(e.g. Tanimoto dissimilarity, scaffold mismatch) instead of only binary top-1 correctness.
Design: `docs/graded-loss-integration.md`. `Candidate`'s public shape is unchanged (the loss is
a caller-supplied `query_id -> f64` map, not a new field); `certify_batch`/
`resolve_realized_losses` keep their exact pre-existing signatures as compatibility wrappers. No
chemistry dependency in `masstrust-core`/`masstrust-cli` — Tanimoto/scaffold computation is a
data-preparation concern, same pattern `benchmarks/massspecgym/run_baseline.py` already uses for
InChIKeys. Not yet validated on real predictions — see "In progress / blocked" below, this is
gated on the same MassSpecGym throughput blocker.

## On `main` (research / benchmark harnesses — not published packages, not version-numbered)

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
- **`benchmarks/dna_adductomics/`**: Phase A literature/data reconnaissance for cancer-relevant
  DNA-adductomics MS/MS selective-annotation, with colibactin as the requested first killer use
  case. **Both the colibactin-specific benchmark and a general-DNA-adductomics benchmark are
  NO-GO** against this project's own pre-registered minimum-n floor — see
  `benchmarks/dna_adductomics/FEASIBILITY.md`. A real-data **pipeline-verification preflight**
  (n=8, explicitly not a benchmark — see `PREFLIGHT_REPORT.md`'s disclaimer) is on `main`:
  reproducible adapter scripts, a network-free `selftest.py`, and checksum-verified data
  download. Nothing here should be cited as a DNA-adductomics selective-annotation *result*.

## In progress / blocked

- MassSpecGym official seed-0 benchmark run (throughput-blocked, see above). Also now blocks
  validating graded chemical loss (`certify-batch --loss-column`, see "Released" above) against
  real predictions with real candidate identity — `docs/graded-loss-integration.md`'s
  verification plan needs exactly this run's output (full candidate SMILES, not just top-1) to
  compute a real Tanimoto/scaffold loss column against.
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
