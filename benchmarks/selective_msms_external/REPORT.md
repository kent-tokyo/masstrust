# Selective-MSMS external-prediction compatibility benchmark: results

**Not a competitor-parity benchmark. Not the official MassSpecGym v1.5 benchmark. Does not
reproduce Selective-MSMS's own split.** See `README.md` for full scope and the forbidden-framing
list this report adheres to.

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

## Known assumption violation

The molecule-grouped split intentionally makes calibration and evaluation disjoint on an
attribute (`molecule_group_id`) correlated with confidence/correctness. This means the
exchangeability assumption underlying SCoRE-SDR's certificate (and, informally, the other
methods' calibration-transfers-to-evaluation logic) does not formally hold here. It applies
identically to all five methods below — see `README.md` for the full statement. **None of the
numbers in this report should be read as a formally verified guarantee on this exact split.**

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

**Selection is thin everywhere, and that is the finding, not a bug.** With an 86.4% top-1 error
rate, none of the five methods can accept a meaningful fraction of queries at alpha ∈
{0.01, 0.05, 0.10} without violating (or, for SDR, risking) their target. Coverage tops out at
0.76% (SDR coupled, alpha=0.10). This is consistent with the instructions that opened this
phase: *"0件選択は実装失敗ではありません。保証付き方式が現在のconfidence scoreではpower不足だった、
という重要なbenchmark結果です"* — zero (or near-zero) selection reflects that the confidence
score has too little separating power at this base error rate, not a defect in either the
legacy or risksieve-backed implementation.

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

**`risksieve SDR independent` selected nothing at any alpha, including where `coupled` selected
69.** This matches theory: the independent construction (Equation 4.1) scores each test point
using only its own e-value against the calibration set, discarding the cross-test-point
information the coupled construction (Equation 5.1) uses — strictly less powerful by
construction, and it shows here as a real power gap at this batch size, not just an asymptotic
one.

**`risksieve SDR independent`'s runtime (160–250s) vs. `coupled`'s (0.45–0.61s) is a genuine,
reportable cost, not a benchmark artifact.** `certify_independent` calls
`risk_adjusted_evalue` once per test point, and that function re-sorts the full calibration set
(plus the test point) on every call — `O(n_test × n_cal log n_cal)` — versus `coupled`'s
documented `O(n + m)` single scan (see `docs/risksieve-integration.md`, "`certify` (the default,
paper-exact coupled construction...)"). At `n_cal=8,520, n_test=9,036` that is roughly
`9,036 × 8,520 × log₂(8,520) ≈ 1×10⁹` comparisons, consistent with the observed ~3–4 minute
wall time. `docs/risksieve-integration.md` notes `certify_independent` "does not have
[the batch-composition-dependence] property... but is still a one-batch decision, not a reusable
threshold" — it does not comment on relative performance; the cost measured here is new
information from this benchmark, not a restatement of that doc.

## What this does not show

This does not show masstrust "beating" or "losing to" Selective-MSMS — Selective-MSMS's own
published SGR numbers are a different question (see `benchmarks/selective_msms/PLAN.md`, "What
can be reproduced from their artifacts without any new engineering"), not evaluated here. This
does not show an AURC-equivalent metric for the SDR methods — SDR selection is not comparable to
a risk-coverage curve point by point, only its selected-count/coverage/realized-risk tuple at
each alpha, which is what's reported above. This does not validate SDR's certificate (a realized
batch's risk landing above or below alpha does not confirm or refute the theorem).
