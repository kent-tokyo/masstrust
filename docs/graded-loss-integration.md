# Graded chemical loss for risksieve-backed certification

## Status

**Implemented** (PR #9, merged as `3c81139`). This memo is kept as the historical design record —
the reasoning below (responsibility split, why not compute chemistry in Rust, why legacy
`calibrate`/`evaluate` is out of scope) is unchanged and still accurate. Two things changed
between this memo and what actually shipped, driven by code review on the first implementation
attempt: see "Implementation deviations from this memo" at the end. Read the doc comments on
`risksieve_backend::LossSource`/`certify_batch_with_loss`/`resolve_realized_losses_with_loss` in
`crates/masstrust-core/src/risksieve_backend/mod.rs` for the authoritative current API, not this
memo's original sketch below.

Scopes a feature-gated (`risksieve` feature, no new default-feature surface) generalization of
`certify-batch`'s loss from binary top-1 correctness to any `[0,1]`-bounded loss, so a caller can
certify against a graded chemical loss (Tanimoto dissimilarity, scaffold mismatch, a hybrid)
instead of only exact-match/no-match. This is **not** a change to `risksieve` itself —
`risksieve`'s SDR guarantee is already stated for a generic `ClosedUnitInterval` loss; masstrust
has just never constructed one other than correctness so far. It is also **not** a change to
legacy `calibrate`/`evaluate` — see "Why legacy calibrate/evaluate is out of scope" below.

## Why this exists

The Selective-MSMS external query-confidence benchmark
(`benchmarks/selective_msms_external/REPORT.md`) found that at an ~86% top-1 error rate, every
risk-control method masstrust has — legacy and risksieve-backed alike — accepts almost nothing at
alpha ≤ 0.10. Part of that is real (a weak upstream retriever), but part of it is an artifact of
how masstrust defines "error" at all: `is_correct` is exact-InChIKey match, full stop. A candidate
that is the right scaffold with the wrong stereocenter, or a close positional isomer, currently
costs exactly as much loss (1.0) as a molecule with no structural relationship to the target. For
a chemistry audience that distinction is often the entire point — "how wrong" matters, not just
"wrong or not." Letting a user choose the loss granularity and still get a certified bound on it
is a real product differentiator, not a Selective-MSMS-following move (Selective-MSMS's own
released artifacts are exact-match throughout too).

## Responsibility split

**`risksieve` owns**, unchanged by this work: the certificate math for any `[0,1]` loss — e-values,
eBH selection, certificate assembly, `ClosedUnitInterval` validation. `risksieve` has no concept
of "chemistry"; it validates a bounded real number, nothing more. This integration asks nothing
new of `risksieve`.

**masstrust owns**, extended by this work: constructing that `[0,1]` loss from either (a) today's
binary correctness (`is_correct`), unchanged and still the default, or (b) a precomputed numeric
column supplied by the caller. masstrust does not compute chemistry itself — see next section.

## Why not compute chemistry in Rust

Chemical loss computation (Tanimoto similarity from fingerprints, scaffold extraction) stays
entirely outside `masstrust-core` and `masstrust-cli`. Concretely: `masstrust-core` gains no new
dependency for this feature, cheminformatics or otherwise.

This isn't a "keep it simple for now, revisit later" choice — it follows directly from where the
project's own cheminformatics work already lives. `benchmarks/massspecgym/run_baseline.py`
already computes InChIKeys from SMILES via RDKit (`MolToInChIKey`), in Python, as part of data
preparation, before anything reaches masstrust's Rust CLI. Tanimoto/scaffold loss computation is
the same kind of step — a data-preparation concern, producing a plain numeric column — not a
concern of the certification engine that consumes it. Keeping it there means:

- `masstrust-core`/`masstrust-cli` stay chemistry-agnostic, with no RDKit build/FFI/licensing
  surface to maintain.
- The loss a caller certifies against is inspectable, versionable, and reproducible as an
  ordinary CSV/Parquet column, the same way `confidence`/`score` already are.
- `TanimotoLoss`/`ScaffoldLoss` are *labels* a caller attaches to a precomputed column (for
  report/certificate provenance — "this certificate is about scaffold-mismatch risk," not "this
  certificate is about exact-match risk"), not Rust implementations of cheminformatics
  algorithms.

Live in-process chemical loss computation (an optional Rust cheminformatics backend) is
explicitly not ruled out forever, but is out of scope for this design and would need its own
memo if ever proposed — see "Open items" below.

## Why legacy calibrate/evaluate is out of scope

Binary correctness is far more deeply embedded in the legacy path than in `certify-batch`.
`metrics/risk_coverage.rs`'s `compute_curve`/`evaluate_at_threshold`/`RiskCoverageRow` all count
`errors: usize` — an integer count of `!is_correct` — not a sum of a continuous loss.
`CalibrationMethod::Binomial` computes a one-sided Wilson score interval, a concentration
inequality specifically for a **Bernoulli** proportion (`calibration::wilson_upper_bound`).
Generalizing `binomial` to a graded `[0,1]` loss is not a data-model change — a Wilson interval
does not apply to a non-Bernoulli mean. It would need a different concentration inequality
entirely (e.g. an empirical-Bernstein or Hoeffding bound for bounded random variables), which is
a real statistical design decision, not incidental plumbing, and is deliberately not made here.

`certify-batch` has no equivalent problem: SCoRE-SDR's guarantee (`risksieve::selective::sdr`) is
already stated for a generic `ClosedUnitInterval` loss (see `docs/risksieve-integration.md`'s
estimand table). Accepting a graded loss there is a natural extension of an interface that
already expects one, not a foundational change. This is also consistent with
`docs/risksieve-integration.md`'s existing boundary that `certify-batch` is an independent
workflow from `calibrate`/`apply` — this work does not blur that boundary in the other direction
either.

## `LossSource` design

**This section is the original plan and does not match what shipped — see "Implementation
deviations from this memo" at the end.** Kept verbatim as the historical record of the starting
point; the deviation was found during code review of the first implementation attempt, not
during this memo's own drafting.

Generalizes today's hardcoded pair in `risksieve_backend/mod.rs`:
`correctness_loss(is_correct: bool) -> ClosedUnitInterval`, fed by `top1_is_correct(ranking)`
(which reads `Candidate.is_correct: Option<bool>`), called from exactly two places —
`certify_batch()`'s calibration-loss loop and `resolve_realized_losses()`.

```rust
pub enum LossSource<'a> {
    /// Today's behavior. The default — every existing caller and test is unaffected.
    BinaryCorrectness,
    /// Read a precomputed `[0,1]` loss from this named column (e.g. "tanimoto_loss",
    /// "scaffold_loss", "hybrid_loss" — the name is caller-chosen and carried into the
    /// certificate/report verbatim, masstrust does not interpret it chemically).
    PrecomputedColumn(&'a str),
}
```

`Candidate` gains one new field, additive to the existing schema:

```rust
pub loss: Option<f64>,
```

Populated from a new optional CSV/Parquet column. Existing CSVs with no such column are
unaffected — `PrecomputedColumn` is opt-in per invocation via the CLI flag below, and
`BinaryCorrectness` never reads this field at all. A `PrecomputedColumn` query with `loss: None`,
or a value outside `[0, 1]`, or non-finite, is a hard error (new `MasstrustError` variants,
following the existing `NonFiniteConfidence`/`MissingCalibrationLabel` precedent of "never
silently treat missing or invalid input as a default value or an abstain") — never a silent clamp
to `0.0`/`1.0` and never a silent exclusion. Exact variant names/wording are an
implementation-phase detail, not a memo-blocking decision — candidates: `MissingLossColumn`,
`LossOutOfRange`.

`certify_batch()` and `resolve_realized_losses()` take a `LossSource` parameter; both call sites
that currently hardcode `correctness_loss(top1_is_correct(ranking)...)` route through it instead.
No other function signature changes.

## CLI surface

```
masstrust certify-batch \
  --calibration calibration.csv --test test.csv \
  --score max-prob --alpha 0.10 --gamma 0.10 \
  --loss-column tanimoto_loss
```

`--loss-column` is optional; omitting it preserves exactly today's behavior
(`LossSource::BinaryCorrectness`, reading `is_correct`). When given, `certify_batch` uses
`LossSource::PrecomputedColumn(name)` and the named column must be present and valid on every
scoreable calibration query (same "hard error, not silent exclusion" rule `is_correct` already
follows) — realized-risk resolution (`resolve_realized_losses`) uses the same column on the
labeled batch passed to it.

**As shipped, `--loss-column` on `--calibration` is required; on `--test` it's genuinely
optional** — a `--test` file with no such column at all certifies successfully (SCoRE-SDR's
certificate never needed a test-side loss, only a test-side score), it just can't produce a
post-hoc realized-risk number. See "Implementation deviations from this memo" below; this
distinction isn't in the original plan sketched above.

## Report/certificate provenance

`certified_population()` already states *which scoring method* a certificate's guarantee applies
to (`"queries scoreable under {ScoringMethod:?}"` — `risksieve_backend/mod.rs`). This needs a
parallel, equally explicit statement of *which loss* was certified, so a report can never be
misread as certifying exact-match risk when it actually certified scaffold-mismatch risk (or vice
versa). Concretely: `certificate.json` and `report.md` gain a `loss_source` field/line — literally
`"binary_correctness"` or `"precomputed_column: tanimoto_loss"` — displayed with the same
prominence `score_orientation_note`/`query_ordering_policy` already have. A report must never
present a `PrecomputedColumn`-certified bound as "the error rate" without qualification.

## Verification plan

`query_scores.parquet` (the just-completed benchmark's data source) cannot be reused for this —
it has no per-candidate identity, only top-1 correctness, so there is nothing to compute a
Tanimoto or scaffold loss *against*. masstrust's own MassSpecGym harness
(`benchmarks/massspecgym/run_baseline.py`) is the only currently-available data source with real
candidate identity (full SMILES for every candidate, not just top-1, plus RDKit-computed
InChIKeys already in the pipeline) — verification of this feature must run there, computing
Tanimoto/scaffold loss columns from that existing SMILES data, once masstrust's own training
throughput blocker is resolved enough to produce real predictions to certify against. The
resulting comparison should report coverage under exact-identity risk vs.
fingerprint-dissimilarity risk vs. scaffold-mismatch risk on the same split, so the value of
graded loss is demonstrated on masstrust's own predictions, not retrofitted onto external data
that was never suited to it.

## Open items deferred to later phases, not resolved here

- Legacy `calibrate`/`evaluate` (`empirical`/`binomial`/`crc`) graded-loss support — needs its own
  design decision on which concentration inequality replaces Wilson for `binomial`; not started
  here.
- A live, in-process cheminformatics backend (optional RDKit dependency inside masstrust) as an
  alternative to precomputed columns — not ruled out permanently, but not designed here; would
  need its own memo if ever proposed.
- The hybrid-loss weighting scheme (how identity/scaffold/similarity combine into one number) is
  a data-preparation concern for whoever produces the precomputed column, not something
  `masstrust-core` opines on — `LossSource::Precomputed` is agnostic to how the map was built.
- ~~Exact `MasstrustError` variant names/messages for missing-column and out-of-range cases.~~
  Resolved: `MissingLossColumn`, `LossOutOfRange`, `InvalidLossValue` (malformed, distinct from
  missing), `LossSourceMismatch` (added beyond this memo's original scope — see below).

## Implementation deviations from this memo (found during code review, PR #9)

The first implementation attempt followed this memo's `LossSource`/`Candidate.loss` sketch
literally and was rejected on review for a real defect it introduces: `Candidate` is a `pub`
struct re-exported from `masstrust-core`'s crate root, and every one of its fields is `pub`.
Adding `pub loss: Option<f64>` to it is **source-breaking** for any downstream crate that
constructs a `Candidate` via a struct literal (as `masstrust-core`'s own `io.rs`, `policy.rs`,
`metrics/risk_coverage.rs`, and every `scoring/*.rs` file already do) — this memo's "additive,
existing callers unaffected" claim was wrong for Rust struct literals specifically, even though
it's true for the CSV/Parquet schema those fields are read from.

What actually shipped instead, and why it still satisfies every requirement above:

- **No `Candidate` change, at all.** `LossSource::Precomputed { label: &str, values_by_query: &
  BTreeMap<String, f64> }` replaces `PrecomputedColumn(&str)` — the loss lives in a `query_id ->
  f64` map the caller builds and passes in, not a field the type is extended with. New
  `io::read_query_loss_column(path, col) -> BTreeMap<String, f64>` builds it directly from
  `(query_id, rank, named column)`, keeping only rank-1 rows — this also fixes a
  "read-a-row-list-and-zip-by-index" fragility the original `Candidate.loss` plan would have
  inherited from reusing `read_group_column`'s row-order contract.
- **`certify_batch`/`resolve_realized_losses` keep their exact original signatures**, now
  implemented as `LossSource::BinaryCorrectness`-fixed compatibility wrappers around new
  `certify_batch_with_loss`/`resolve_realized_losses_with_loss`. This memo's "no other function
  signature changes" line was also wrong in the first attempt (both gained a new required
  parameter) — the wrapper pattern restores it for real.
- **The test side of `certify_batch_with_loss` never needs a loss**, precomputed or otherwise —
  it only ever needed a *score* (true before this feature existed too). This memo didn't say
  otherwise, but the first implementation's CLI wiring accidentally required `--loss-column` to
  exist in `--test` too, which would have made a genuinely unlabeled test set an error. Fixed:
  `--test` reading the loss column is optional, and `MasstrustError::MissingColumn` from that
  specific read is treated as "no test labels" rather than propagated.
- **Added beyond this memo's original scope**: `BatchCertification.loss_kind` (typed, not just
  the `loss_source` string this memo planned) lets `resolve_realized_losses_with_loss` reject
  (`MasstrustError::LossSourceMismatch`) being asked to resolve realized risk under a *different*
  loss than what was actually certified.
- Report/certificate provenance shipped as four fields (`loss_kind`, `loss_label`, `loss_column`,
  `loss_domain`) rather than this memo's single `loss_source` string — same intent (never
  presentable as exact-match risk without qualification), more machine-readable.
