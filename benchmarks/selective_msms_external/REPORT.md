# Selective-MSMS external query-confidence benchmark: results

**Not a competitor-parity benchmark. Not the official MassSpecGym v1.5 benchmark. Does not
reproduce Selective-MSMS's own split. Does not import or reconstruct candidate-level rankings —
this benchmark operates on query-level confidence and correctness only.** See `README.md` for
full scope (including the "What this benchmark does/does not demonstrate" lists and `scope`
metadata) and the forbidden-framing list this report adheres to.

Data: `data/results/numerical/query_scores.parquet` from Selective-MSMS's Zenodo release
(record 19108280), filtered to `run_label == "mlp_mass"`, `split == "test"`, `K == 1`
(17,556 queries, 2,998 unique target molecules). Confidence score: the artifact's own
`confidence` column, fed to masstrust unchanged as `score`/`probability` (`--score max-prob`).
**Base rate: Hit@1 = 0.140636** (86.4% top-1 error rate) — shown next to every coverage number
below, since it is the ceiling against which "how much did we accept" should be read.

## Split

Group split by `molecule_group_id` (== 14-char 2D InChIKey block), seed 42, ~50/50 by group
count. `masstrust validate-split` confirms 0 query_id overlap and, more to the point, **0
target-InChIKey overlap** (`target_inchikey_overlap: 0`, `target_inchikey_skeleton_overlap: 0` —
the WARNING branch that fires on target-molecule leakage never triggered): no target molecule's
queries appear in both halves, which is what the grouped split was built to guarantee.
(`validate-split`'s `candidate_pool_overlap` also reports 0, but that check is vacuous here by
construction — each row's `candidate_id` is a synthetic `{query_id}_top1`, so the calibration and
evaluation candidate-ID sets are disjoint regardless of which queries land in which half. It is
not evidence of anything and is omitted from the table below.) Full assignment in
`split_manifest.json` (immutable, written before any method below was run).

| | queries | molecule groups |
|---|---|---|
| calibration (`masstrust_calibration_half`) | 8,520 | 1,499 |
| evaluation (`masstrust_evaluation_half`) | 9,036 | 1,499 |

Input hashes: `calibration.csv` sha256 `49e526a7d4...`, `evaluation.csv` sha256 `d5d43ab298...`,
source `query_scores.parquet` sha256 `f8535c615d...` (matches Zenodo's own `MANIFEST.tsv`).

**No queries were unscoreable in this run** (every row has a populated `probability`/`score` by
construction — see `README.md`'s note on why this artifact reduces to one confidence value per
query). `calibration_scoreable = calibration_total = 8,520`,
`test_scoreable = test_total = 9,036` for every method below.

## Known limitation: whole-batch exchangeability is not established

Query-level joint exchangeability is not established because multiple spectra are clustered
within target molecules. The molecule-grouped split prevents target leakage, but does not by
itself establish the whole-batch exchangeability required by SCoRE-SDR. The group split was
chosen to prevent target leakage, not to engineer a particular difficulty distribution; group
membership was assigned by an unweighted random shuffle (seed 42), not selected to produce a
deliberate difficulty shift. `risksieve` computed the certificate objects below correctly from
the inputs given to them — this is not a claim of a bug in `risksieve`. What is not established
is that this benchmark's own data-generating structure (clustered, molecule-correlated spectra)
satisfies the theorem's exchangeability hypothesis. This applies identically to all five methods
below, so it does not favor any one of them — see `README.md` for the full statement. Treat this
as an **assumption-unverified diagnostic benchmark**: none of the numbers in this report should
be read as a formally verified guarantee on this exact split.

## Results

alpha = gamma throughout. Binomial confidence level fixed at 0.95 (pre-registered, not swept).
For the three threshold methods, "calibration" columns are what `masstrust calibrate` found on
the calibration half (8,520 queries); "evaluation" columns are that same fixed threshold applied
to the held-out evaluation half (9,036 queries) via `masstrust evaluate` — shown side by side so
the calibration→evaluation gap below is checkable, not asserted.

| alpha | method | cal accepted/total | cal risk | eval accepted/total | eval coverage | eval risk (realized) | risk vs target | runtime (s) |
|---|---|---|---|---|---|---|---|---|
| 0.01 | empirical | 18/8520 | 0.0000 | 34/9036 | 0.00376 | 0.0588 (2/34) | **exceeds target** (0.0588 > 0.01) | 0.035 |
| 0.01 | binomial (95% Wilson) | 0/8520 (+inf) | n/a | 0/9036 | 0.0 | n/a (abstain-all) | n/a | 0.036 |
| 0.01 | legacy-crc | 18/8520 | 0.0000 | 34/9036 | 0.00376 | 0.0588 (2/34) | **exceeds target** (0.0588 > 0.01) | 0.030 |
| 0.01 | risksieve SDR, coupled | — (joint, not a threshold) | — | 0/9036 | 0.0 | realized 0.0 | n/a (0 selected) | 0.52 |
| 0.01 | risksieve SDR, independent | — | — | 0/9036 | 0.0 | realized 0.0 | n/a (0 selected) | 249.97 |
| 0.05 | empirical | 18/8520 | 0.0000 | 34/9036 | 0.00376 | 0.0588 (2/34) | **exceeds target** (0.0588 > 0.05) | 0.032 |
| 0.05 | binomial (95% Wilson) | 0/8520 (+inf) | n/a | 0/9036 | 0.0 | n/a (abstain-all) | n/a | 0.029 |
| 0.05 | legacy-crc | 18/8520 | 0.0000 | 34/9036 | 0.00376 | 0.0588 (2/34) | **exceeds target** (0.0588 > 0.05) | 0.030 |
| 0.05 | risksieve SDR, coupled | — | — | 0/9036 | 0.0 | realized 0.0 | n/a (0 selected) | 0.61 |
| 0.05 | risksieve SDR, independent | — | — | 0/9036 | 0.0 | realized 0.0 | n/a (0 selected) | 219.54 |
| 0.10 | empirical | 27/8520 | 0.0741 (2/27) | 55/9036 | 0.00609 | 0.0909 (5/55) | within target (0.0909 < 0.10) | 0.032 |
| 0.10 | binomial (95% Wilson) | 0/8520 (+inf) | n/a | 0/9036 | 0.0 | n/a (abstain-all) | n/a | 0.032 |
| 0.10 | legacy-crc | 27/8520 | 0.0741 (2/27) | 55/9036 | 0.00609 | 0.0909 (5/55) | within target (0.0909 < 0.10) | 0.031 |
| 0.10 | risksieve SDR, coupled | — | — | **69**/9036 | 0.00764 | realized **0.1159** | above target on this realized batch (0.1159 > 0.10; not a certificate violation — see below) | 0.45 |
| 0.10 | risksieve SDR, independent | — | — | 0/9036 | 0.0 | realized 0.0 | n/a (0 selected) | 160.34 |

At alpha ∈ {0.01, 0.05}, `empirical`/`legacy-crc` find the identical calibration threshold
(0.998955) accepting 18/8520 with **0 observed errors on calibration** — the same threshold then
accepts 34/9036 on evaluation with 2 errors (0.0588). That 0-error → 2-error jump on a ~20-30-item
accepted set is the sampling-variance story below, now visible rather than asserted. Calibration
curve (constant across alpha, a property of the full calibration set): AURC 0.734250, E-AURC
0.141181.

`risksieve_sdr_*`: `guarantee_kind = SelectiveDeploymentRisk` in every run, `certified_upper_bound
== alpha` in every run — confirmed directly from `risksieve`'s `assemble_certificate` (`sdr.rs`:
`certified_upper_bound: alpha.get()`, i.e. always exactly alpha for this guarantee kind, not a
computed bound), `certified_population = "queries scoreable under MaxProb"`. `certify-batch`
consumes both CSVs jointly per call rather than calibrating a reusable threshold, so it has no
calibration-only row to show.

## Reading these numbers

**Summary of the main result:** Hit@1 ≈ 14.1% (top-1 error ≈ 85.9–86.4%). At alpha ≤ 0.10, every
method's selection is thin — coverage tops out at 0.76% (SDR coupled, alpha=0.10). `binomial`'s
abstain-all at every alpha is a normal, fail-closed result, not a malfunction. `empirical`/
`legacy-crc`'s evaluation-side risk excess at low alpha is a descriptive result reflecting
small-sample variance (see the calibration-vs-evaluation columns above), not a proof either
method is broken. SDR coupled's single realized batch landing above alpha=0.10 is not a
certificate violation (see below). SDR independent's 0-selection here is this fixture's result,
not a general statement about which construction has more power (see above).

**Selection is thin everywhere, and that is the finding, not a bug.** With an ≈86% top-1 error
rate, none of the five methods can accept a meaningful fraction of queries at alpha ∈
{0.01, 0.05, 0.10} without violating (or, for SDR, risking) their target. This is consistent with
the instructions that opened this phase: *"0件選択は実装失敗ではありません。保証付き方式が現在の
confidence scoreではpower不足だった、という重要なbenchmark結果です"* — zero (or near-zero)
selection reflects that the confidence score has too little separating power at this base error
rate, not a defect in either the legacy or risksieve-backed implementation.

**`empirical` and `legacy-crc` are identical at every alpha tested.** CRC's finite-sample
correction is `1/(n+1) ≈ 0.000117` at n=8,520 — small enough that it didn't move the
max-coverage threshold search away from empirical's answer on this data at these targets.

**`empirical`/`legacy-crc` exceed their target risk on the evaluation half at alpha ∈
{0.01, 0.05}** (observed 0.0588 vs. targets 0.01/0.05). This is expected and is exactly what
these two calibration methods do NOT guard against: both calibrate a threshold from the
*calibration* half's observed risk, with no distribution-free bound protecting the
*evaluation* half from sampling variance at n=34 accepted — a small accepted count where a
couple of misses swing the observed rate by several points. `binomial`'s Wilson-bound approach
is the one designed to be conservative against exactly this, and it responded by abstaining
entirely (threshold = +inf) at all three alphas rather than accept a small set it can't bound
tightly enough.

**`risksieve SDR coupled`'s realized risk (0.1159) exceeding its alpha=0.10 target on this one
batch is not a certificate violation.** `certify-batch`'s own report language states this
explicitly: SelectiveDeploymentRisk is a bound on the expectation over the joint draw of
calibration and the *entire* test batch, not a per-realized-batch guarantee — a single draw
landing above alpha is consistent with the theorem, the same way a 95% CI can fail to cover on
any one draw. `realized_selective_risk` is reported here as the descriptive statistic it is, not
as evidence for or against the certificate.

**On this dataset and pre-registered configuration, the independent construction selected fewer
queries than the coupled construction (0 vs. 69 at alpha=0.10; 0 vs. 0 at alpha ∈ {0.01, 0.05}).
Neither construction is known to dominate the other in general** — `risksieve` does not claim
general dominance either way, and this benchmark does not either. The independent construction
(Equation 4.1) scores each test point using only its own e-value against the calibration set,
discarding the cross-test-point information the coupled construction (Equation 5.1) uses; that
difference in what information each construction uses is a documented design distinction, not by
itself a proof that one always selects at least as much as the other. What's reported here is
this fixture's outcome, not a general power ranking.

**`risksieve SDR independent`'s runtime (single-run measurement: 160–250s, this hardware/build)
vs. `coupled`'s (single-run measurement: 0.45–0.61s) is a genuine, reportable cost on this
fixture, not a benchmark artifact — but the ~300–500× ratio itself should not be read as a
general or theoretical constant.** Verified directly against `risksieve` 0.2.0's source
(`src/selective/sdr.rs`, `src/selective/evalue.rs`, `src/selective/coupled.rs`), not asserted:

- `certify_independent` calls `risk_adjusted_evalue` once per test point (`m` calls). Each call
  builds a `candidates` list of up to `O(n)` breakpoint values and, for every candidate, does a
  `.rev().find(...)` linear scan over the `O(n)` grouped calibration values
  (`largest_feasible_index`) — `O(n)` candidates × `O(n)` scan = **`O(n²)` per call**, on top of
  an `O(n log n)` sort that is not the dominant term. Total: **`O(m·n²)`**.
- `certify` (coupled) sorts the pooled calibration+test scores once (`O((n+m) log(n+m))`), then
  for each of the `m` test points does an `O(n+m)` linear scan (`t0`/`t1` search) and, on the
  (data-dependent) branch where a tie-breaking suffix-maximum is needed, another `O(n+m)` pass —
  **`O((n+m) log(n+m) + m(n+m))`** overall, not the `O(n+m)` this report previously (incorrectly)
  claimed.

Both are super-linear in the batch size; independent's `n²` term is what makes it far more
expensive than coupled's `n+m` term at `n≈8,520`, `m≈9,036`, but this is this fixture's
consequence of those two formulas, not a claim that coupled is always `O(n)`-times cheaper — the
data-dependent suffix-maximum branch in coupled means its typical-case cost can vary by dataset.
Each construction was run once per alpha (not repeated for a median/range) — see "What this does
not show."

## Conclusion

**The dominant bottleneck on this artifact is the weak upstream top-1 accuracy and limited
separation of its confidence score, not the absence of a more permissive downstream controller.**
No amount of calibration/certification-method sophistication recovers meaningful coverage from a
confidence score whose underlying retriever is right on the top pick only ~14% of the time at
these risk targets. This holds identically for masstrust's legacy methods and for
risksieve-backed SCoRE-SDR.

## What this does not show

This does not show masstrust "beating" or "losing to" Selective-MSMS — Selective-MSMS's own
published SGR numbers are a different question (see `benchmarks/selective_msms/PLAN.md`, "What
can be reproduced from their artifacts without any new engineering"), not evaluated here. This
does not show an AURC-equivalent metric for the SDR methods — SDR selection is not comparable to
a risk-coverage curve point by point, only its selected-count/coverage/realized-risk tuple at
each alpha, which is what's reported above. This does not validate SDR's certificate (a realized
batch's risk landing above or below alpha does not confirm or refute the theorem). This does not
show a general power ranking between SCoRE-SDR's coupled and independent constructions, or a
general runtime ratio between them — both are single-fixture, single-run observations (see
above). This does not import, reconstruct, or claim candidate-level ranking identity for this
artifact (see `README.md`, "Scope metadata").
