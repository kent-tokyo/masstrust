# risksieve integration: batch selective-deployment certification

## Status

Design memo for a feature-gated, optional `risksieve` backend that adds theorem-backed
batch accept/abstain certification to masstrust, on top of `risksieve` 0.2.0
([crates.io](https://crates.io/crates/risksieve), MIT OR Apache-2.0). This is **not** a
replacement for masstrust's existing calibration methods (`empirical`, `binomial`, `crc`) —
see "Why not `CalibrationMethod`" below.

## Why this exists

masstrust's existing `calibrate`/`apply` flow produces a *reusable threshold policy*:
calibrate once, apply many times to new unlabeled batches, with either no statistical
guarantee (`empirical`), a conservative one-sided bound (`binomial`), or an experimental
finite-sample correction (`crc`, not proven to the same standard as the other two — see
`README.md`'s calibration methods table).

`risksieve`'s `selective::sdr` module (SCoRE-SDR, Bai and Jin 2026, arXiv:2603.24704,
Algorithm 2 and Theorem 3.3) offers something structurally different: given one labeled
calibration set and **one specific unlabeled test batch**, it returns a selected subset of
that batch with a certified bound on the *average risk among the selected items*, backed by
a published theorem rather than an empirical or ad hoc correction. This integration exposes
that as an independent, opt-in workflow.

## Responsibility split

**`risksieve` owns:**
- theorem-backed risk control (the actual math: e-values, eBH selection, certificate assembly)
- the certificate, its `Assumptions`, and its `Diagnostics`
- validated probability/loss types (`ClosedUnitInterval`, `OpenUnitInterval`, `NonNegative`)
- SCoRE-MDR / SCoRE-SDR, and any future shift-aware controller

**masstrust owns:**
- reading candidate rankings (existing `io::read_candidates` / `io::group_by_query`)
- aggregating to one row per query (existing `QueryRanking`)
- computing confidence scores (existing `scoring::compute_confidence`, reused unchanged)
- constructing the top-1 correctness loss
- scoreable/unscoreable bookkeeping
- mapping `RiskCertificate<Vec<usize>>`'s selected indices back to `query_id`s
- accepted/abstained CSV output and an MS/MS-oriented Markdown report
- displaying the backend's certificate to the user without editorializing on it

masstrust never reimplements or re-derives any of `risksieve`'s formulas. Every number in
the certificate JSON that carries a statistical guarantee comes from `risksieve` verbatim
(via `serde`); masstrust only adds a provenance envelope around it.

## Why not `CalibrationMethod`

Two independent, sufficient reasons, not one:

1. **Different reusability.** `calibrate → policy.json → apply` assumes a policy computed
   once transfers to arbitrary future batches. SCoRE-SDR's certificate is a decision about
   *this specific test batch*, jointly with calibration — see next point.
2. **Batch-composition-dependent selection.** `certify` (the default, paper-exact
   [coupled](https://github.com/kent-tokyo/risksieve/blob/v0.2.0/src/selective/sdr.rs)
   construction, Equation 5.1) computes each test point's e-value using *every other test
   point's score*, not just its own. Removing or adding one query from the batch can change
   which other queries get selected. A "policy" that changes meaning depending on what else
   is in the same submission is not the kind of object `policy.json`/`apply` is designed to
   represent. (`certify_independent`, the pre-5.1 per-test-point-independent construction,
   does not have this property — each test point's e-value depends only on its own score —
   but is still a one-batch decision, not a reusable threshold, so the first reason applies
   to it regardless.)

`masstrust --method crc` is **not touched, renamed, or reinterpreted** by this work. It
remains the existing experimental CRC-style implementation with its own documented caveats.
A future, separately-scoped and separately-audited change could consider whether
`risksieve::crc::monotone::certify` should ever back `CalibrationMethod::Crc` — this
integration makes no such change and expresses no opinion on it.

## Estimand table

Verified against `risksieve` 0.2.0's actual implementation and `docs/guarantees.md`
(commit `ebc6008`, tag `v0.2.0`) — not the papers directly, and not assumed from function
names.

| Method | Controls | Output | Reusable policy |
|---|---|---|---|
| `empirical` | observed conditional error on calibration data | threshold | yes |
| `binomial` | one-sided Wilson upper bound on conditional error | threshold | yes |
| `crc` (legacy, experimental) | empirical target tightened by `1/(n+1)`; not proven to the same standard as `binomial` | threshold | yes |
| `risksieve::crc::monotone::certify` | `E[R(theta_hat)] <= alpha`, `GuaranteeKind::ExpectedRisk` | threshold parameter | yes, in principle — **not wired into masstrust by this integration** |
| `risksieve::selective::mdr::certify` | `E[loss * deploy] <= alpha`, `GuaranteeKind::MarginalDeploymentRisk` — a property of the expectation over the joint draw, not of the single returned decision | one deploy/abstain decision | no — **not wired into masstrust by this integration** |
| `risksieve::selective::sdr::certify` / `certify_independent` | `E[(sum of loss over selected) / (1 v \|selected\|)] <= alpha`, `GuaranteeKind::SelectiveDeploymentRisk` — a property of the expectation over the joint draw of calibration *and the entire test batch*, not of the one realized selected set | selected index set (`RiskCertificate<Vec<usize>>`) | no — **this integration** |

Row 6 is what `certify-batch` wires up. Rows 4-5 are documented here because they are the
other populated `GuaranteeKind` variants in `risksieve` 0.2.0 and it would be misleading to
list SDR without showing what it is not; neither is exposed by any masstrust command.

## Guarantee semantics that must not be blurred

- **Expectation, not per-batch.** `SelectiveDeploymentRisk`'s bound is on `E[... ]` over the
  joint random draw of calibration and the test batch. It does not claim "this batch's
  selected items have risk ≤ alpha" — that would be `EmpiricalOnly`, a different
  `GuaranteeKind` entirely (`docs/guarantees.md` row 7). The CLI report must never say
  anything resembling "the test batch is guaranteed to have error ≤ 5%."
- **Exchangeability scope is the whole batch, not per-query.** `risksieve`'s own docs
  (`docs/assumptions.md`, "Status" section, `selective::sdr` paragraph) state this
  explicitly: SDR requires `{(X_i,Y_i)}_{i=1}^{n+m}` — calibration plus *every* test point —
  to be jointly exchangeable, strictly stronger than the `n+1` requirement every other
  controller in the crate uses. **This has a direct, load-bearing consequence for how
  masstrust must filter unscoreable queries — see next section.** It is not a caveat that
  can be mentioned once and left to the implementation to work out; it constrains the
  implementation.
- **Zero selections is a valid certificate, not an error or a failure.** Confirmed in
  `risksieve`'s own tests (`empty_batch_is_a_valid_empty_certificate`); the SDR bound holds
  trivially via the `1 v |R|` denominator. The CLI must present this as a normal, complete
  result.
- **Realized vs. certified risk are different objects.** `risksieve::selective::sdr::realized_selective_risk`
  already exists for exactly this purpose: a plain `f64`, no `GuaranteeKind` attached, "so it
  cannot be mistaken for a second certificate" (module docs, `sdr.rs`). Phase 5 uses this
  function directly rather than recomputing the ratio in masstrust — reimplementing it would
  forfeit the "not a certificate" type-level distinction that is the library's actual design
  intent, not just a naming convention.
- **`gamma` does not need to satisfy `gamma <= alpha`.** Theorem 4.2 (`evalue.rs` module
  docs) gives validity for any `gamma in (0,1)`; Remark 4.5 (referenced in the same docs) is
  about selection *power*, not validity. `certify-batch` must not add a `gamma <= alpha`
  check that `risksieve` itself does not require — both are independently constructed as
  `OpenUnitInterval` (rejecting exactly `0.0` and `1.0`), and that is the only constraint to
  surface as a CLI error.

## Unscoreable queries: the same predicate on both sides

The spec's original framing — "unscoreable test queries always abstain, never reach
risksieve" — is necessary but not sufficient on its own. Calibration-side queries that are
unscoreable under the chosen method can't produce a calibration score either, so they must
also be excluded, **using the identical scoreability predicate** (the same `ScoringMethod`'s
`compute_confidence` returning `Some`), not a separately-defined rule.

Why this matters, precisely: `compute_confidence(ranking, method)` is a function of the
candidate list only (never of `is_correct`). If test queries are filtered to "scoreable
under `method`" but calibration queries are not filtered the same way, the retained test
population is drawn from a *conditional* distribution (`X | scoreable`) while calibration is
drawn from the *unconditional* one — exactly the kind of caller-introduced, property-based
filtering `risksieve`'s own docs warn invalidates joint exchangeability ("a caller who
assembles a batch by filtering or sorting test points by some property of their own is not
entitled to assume this holds," `sdr.rs` module docs). Applying the same predicate to both
sides keeps calibration and the retained test batch drawn from the same
scoreable-under-`method` sub-population, which is what the certificate is actually about.

Consequences, made explicit rather than left implicit:

- **The certified population is "queries scoreable under method `M`," not "all queries."**
  `certify-batch`'s report must say this plainly, not just report counts.
- Report four counts, not two: calibration total / calibration scoreable / test total / test
  scoreable — plus the excluded reason for each side.
- For `ScoringMethod::CandidateCount`, `compute_confidence` always returns `Some` (`1 /
  n_candidates`, defined for any non-empty candidate list), so this filter is a no-op and the
  certified population is simply "all queries with ≥1 candidate." This is method-dependent,
  not a fixed constant, and the report should reflect the method actually in use.
- The exclusion rule itself (scoreability under `method`) is fixed before any label is
  examined and does not depend on `is_correct` — satisfying the spec's "exclusion rules must
  be label-independent and fixed in advance" requirement by construction, not by a
  separately-argued case.
- Calibration queries missing an `is_correct` label are a **hard error**, not a silent
  exclusion — deliberately different from `metrics::risk_coverage::obs_from_rankings`'s
  existing behavior (which silently filters via `?` in a `filter_map`). That existing
  behavior is appropriate for a descriptive risk-coverage curve; it is not appropriate for
  data feeding a certificate, where an unlabeled row masquerading as absent-by-filtering
  rather than surfaced-as-an-error is exactly the kind of silent scope-narrowing this
  integration exists to avoid elsewhere.
- A `Some(f64)` confidence that is non-finite (NaN or infinite — theoretically possible from
  edge cases in some scoring methods, e.g. `ScoreRatio`) is **not** treated as unscoreable and
  is **not** silently converted to abstain. It is a hard error for the whole `certify-batch`
  invocation by default, consistent with "never silently convert invalid input to abstain
  without an explicit CLI option." No such option is added in this integration; if one is
  wanted later, it must be its own explicit, named flag.

## Score orientation

Confirmed from `risksieve` 0.2.0's own documentation (`evalue.rs`, added in this exact
release in response to this integration's Phase 0 audit — see `docs/references.md` there for
provenance): **"lower is what makes deployment possible... Callers must orient `s(.)` so that
a lower score means a more trustworthy prediction — this convention is not inferred from the
caller's inputs, and getting it backwards silently inverts which points are eligible for
deployment without triggering any validation error."**

masstrust's `ScoringMethod`s are the opposite convention throughout: higher `compute_confidence`
output means higher confidence (`README.md`'s scoring methods table, every `scoring/*.rs`
implementation). The transform is therefore a plain negation:

```text
risksieve_score = -masstrust_confidence
```

This is stated as a fact now that `risksieve` 0.2.0 documents its own convention explicitly,
not a guess — but the spec's instruction to test it rather than trust it by inspection alone
still applies, and Phase 6 includes the property tests it specifies: a high-confidence
correct query is selected more readily than a low-confidence one; a low-confidence incorrect
query is not spuriously selected; score ties resolve deterministically; the orientation
actually applied is recorded in the certificate/report so a future reader does not have to
re-derive it from this document.

## Query ordering

`RiskCertificate<Vec<usize>>::parameter` is documented as sorted-ascending indices into
whatever `test_scores` slice was passed to `certify`/`certify_independent`. masstrust's
existing `io::group_by_query` already returns `Vec<QueryRanking>` sorted by `query_id`
ascending (`HashMap` internally, but keys are collected and `.sort()`-ed before the final
`Vec` is built — `io.rs`, `group_by_query`). The adapter builds its `test_scores` slice by
iterating that same already-sorted, already-filtered-to-scoreable `Vec<QueryRanking>` in
order and does not introduce a second sort or rely on any `HashMap` iteration order of its
own. The certificate/report records this as the ordering policy explicitly (`query_id`
ascending, post-scoreability-filter) rather than leaving it to be inferred.

## `certify-batch` is an independent workflow, not a `calibrate`/`apply` extension

Per the spec: no new `CalibrationMethod` variant, no changes to `PolicyFile`'s schema or
version, no embedding a certificate inside an existing policy. `certify-batch` is a new,
standalone subcommand consuming its own pair of CSVs (calibration + one test batch) and
producing its own output set (`accepted.csv`, `abstained.csv`, `certificate.json`,
`report.md`) each invocation. It is feature-gated behind `risksieve` and not added to any
default feature set.

## Open items deferred to later phases, not resolved here

- The exact `checkpoint_sha256`-style single-hash convention for the certificate's
  provenance envelope when multiple input files are involved is a Phase 4 detail, not a
  Phase 1 architectural question.
- Whether `certificate.json` embeds the full `RiskCertificate` via `serde` or a curated
  subset is Phase 4; the intent (embed it in full plus a masstrust envelope, not a curated
  reinterpretation) is stated above.
