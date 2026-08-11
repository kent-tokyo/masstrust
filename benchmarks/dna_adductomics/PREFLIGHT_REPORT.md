# DNA-adductomics masstrust preflight report

**Exploratory compatibility preflight only. n=8 distinct compounds is below the pre-registered
minimum sample size and these results must not be used as masstrust performance claims or
Coverage@Risk estimates.**

This is a committed snapshot of one real run of `scripts/` (this directory) against the real,
CC-BY 4.0-licensed nexs-metabolomics DNA adductomics database — see `FEASIBILITY.md` §2 for the
dataset, and `FEASIBILITY.md` §0/§2.3/§2.4 for exactly why n=8 fails this project's own
pre-registered minimum-n floor (≥15–20 distinct compounds) and is therefore not a benchmark. What
this run *does* establish: masstrust's adapter → schema → `validate-split` → `calibrate` →
`evaluate` pipeline executes end to end on real, third-party MS/MS data, with zero
`masstrust-core` changes needed to carry the domain provenance columns in `candidates.csv`. Every
number below is real (real spectra, real `matchms` scoring, real `masstrust` CLI output) — the
disclaimer is about what the numbers do *not* license claiming, not about their authenticity.

Reproduce with `python3 scripts/prepare_data.py && python3 scripts/export_candidates.py --data-dir ./data
&& python3 scripts/validate_data.py --data-dir ./data && python3 scripts/run_benchmark.py --data-dir ./data
--out-dir ./report && python3 scripts/generate_report.py --data-dir ./data --report-dir ./report` (see
`README.md`). `report/` (including a regenerated copy of everything below) is gitignored and
regenerated fresh by that command; this file is the one committed, point-in-time record.

## Provenance

| field | value |
|---|---|
| dataset | nexs-metabolomics DNA adductomics database, CC-BY 4.0 |
| dataset source | `gitlab.com/nexs-metabolomics/projects/dna_adductomics_database` |
| dataset pinned commit | `15db61a372676fd6fa5e64b2076681a41f187cf4` |
| dataset file hashes | `database.xlsx` sha256 `1631952f…c2c42`, `experimental.html` sha256 `36e7d0dd…d6d4f`, `predicted.html` sha256 `942f249e…3e13b` (full hashes: regenerate `data/manifest.json`) |
| masstrust commit | `a44aa3fa38ba43cd4ea77890b70cb8a14d4eb4c4` (PR #7 merge — the commit this preflight was run against) |
| external scorer | `matchms` 0.33.1, `CosineGreedy(tolerance=0.01)` |
| candidate pool | mass-filtered ±0.5 Da on charged monoisotopic mass (not load-bearing at this scale — identical pool sizes from ±0.1 Da through ±0.5 Da) |
| split | deterministic alphabetical-by-InChIKey first-half/second-half, compound-disjoint (4 calibration / 4 test) — arbitrary, not seeded-random, because n is too small for randomization to matter (see `FEASIBILITY.md` §0/§2.3) |
| run_kind | `preflight` (stamped in every CSV row and every generated report) |

## Dataset

8 real experimental query compounds (LC-MS/MS on a Waters Vion IM-QTOF, ESI+, CE 20/40/60/80 eV),
real CFM-ID-predicted candidate pools drawn from 579 candidate compounds with valid InChIKeys.
Compound-disjoint calibration (4 compounds) / test (4 compounds) split, verified leak-free by
`masstrust validate-split` (0/4 query_id overlap).

**Label-conflation check** (is `is_correct=false` ever really a stereoisomer/duplicate of the true
compound rather than a genuinely different molecule?): for every query where the top-1 pick was
wrong, its InChIKey's first-14-character 2D-skeleton block was compared against the true
compound's — the same check `benchmarks/massspecgym/` uses for leakage. **All 4 wrong top-1 picks
have a different skeleton block**; none is a stereoisomer of the answer. The accuracy/risk numbers
below are not a labeling artifact.

## Baseline: accept-all top-1 accuracy (all 8 queries, descriptive only)

**4/8 correct (50%)** — real matchms cosine top-1 pick vs. the true compound. Not a headline
number: n=8.

| query (InChIKey) | genotoxicant class | n candidates | top-1 correct? | top-1 score |
|---|---|---|---|---|
| BCKDNMPYCIOBTA-FSDSQADBSA-N | Alkylation, NOC | 5 | yes | 0.638 |
| DYSDOYRQWBDGQQ-BWZBUEFSSA-N | Alkylation, NOC | 5 | **no** | 0.554 |
| HCAJQHYUCKICQH-UOWFLXDJSA-N | ROS | 7 | **no** | 0.320 |
| INAGNQRTDPXONR-AIPQFCGWSA-N | Acrolein | 7 | **no** | 0.554 |
| JPQHAFBEGLGQRF-BRWVUGGUSA-N | AA | 2 | **no** | 0.379 |
| KIAMDXBTZWMIAM-IWSPIJDZSA-N | Alkylation, LPO | 11 | yes | 0.584 |
| LUCHPKXVUGJYGU-BWZBUEFSSA-N | Alkylation | 2 | yes | 0.738 |
| QZDHOBJINVXQCJ-IWSPIJDZSA-N | Malonaldehyde | 2 | yes | 0.377 |

## Baseline method comparison (`masstrust compare`, full 8-query set, exploratory)

AURC/E-AURC with bootstrap CI (n=200 resamples) across masstrust's four score-only confidence
methods. `matchms` cosine is not a calibrated probability — `max-prob`/`margin`/`entropy`/
`effective-k` are not applicable, same reasoning as `benchmarks/massspecgym/README.md`.

| method | threshold | accepted/total@0.10 | AURC | E-AURC | AURC 95% CI |
|---|---|---|---|---|---|
| score-gap | 0.363 | 1/8 | 0.351 | 0.197 | [0.072, 0.728] |
| score-ratio | 1.97 | 1/8 | 0.382 | 0.228 | [0.073, 0.866] |
| topk-gap | 0.249 | 2/8 | 0.288 | 0.135 | [0.072, 0.720] |
| candidate-count | n/a | n/a | 0.343 | 0.189 | [0.070, 0.770] |

## Calibrate (4 compounds) → evaluate held-out (4 compounds), all score × method × target

Every combination actually run, including outright failures, per the small-n honesty requirement.
`abstain_reason` is masstrust's own machine-generated explanation when a policy accepts nothing.

| score | calibration method | target risk | accepted/total (coverage) | realized risk | Wilson UB | note |
|---|---|---|---|---|---|---|
| score-gap | empirical | 0.05 | 2/4 (0.50) | 0.500 **exceeded** | 0.879 | |
| score-gap | empirical | 0.1 | 2/4 (0.50) | 0.500 **exceeded** | 0.879 | |
| score-gap | empirical | 0.2 | 2/4 (0.50) | 0.500 **exceeded** | 0.879 | |
| score-gap | binomial | 0.05 / 0.1 / 0.2 | 0/4 (0.00) | n/a (0 accepted) | n/a | 0/4 labeled queries met threshold +inf — check `masstrust drift`, or whether the threshold is too strict for this scoring method on this data |
| score-ratio | empirical | 0.05 | 2/4 (0.50) | 0.500 **exceeded** | 0.879 | |
| score-ratio | empirical | 0.1 | 2/4 (0.50) | 0.500 **exceeded** | 0.879 | |
| score-ratio | empirical | 0.2 | 2/4 (0.50) | 0.500 **exceeded** | 0.879 | |
| score-ratio | binomial | 0.05 / 0.1 / 0.2 | 0/4 (0.00) | n/a (0 accepted) | n/a | same as above |
| topk-gap | empirical | 0.05 | 1/4 (0.25) | 0.000 ok | 0.730 | |
| topk-gap | empirical | 0.1 | 1/4 (0.25) | 0.000 ok | 0.730 | |
| topk-gap | empirical | 0.2 | 1/4 (0.25) | 0.000 ok | 0.730 | |
| topk-gap | binomial | 0.05 / 0.1 / 0.2 | 0/4 (0.00) | n/a (0 accepted) | n/a | same as above |
| candidate-count | empirical | 0.05 / 0.1 / 0.2 | 0/4 (0.00) | n/a (0 accepted) | n/a | same as above |
| candidate-count | binomial | 0.05 / 0.1 / 0.2 | 0/4 (0.00) | n/a (0 accepted) | n/a | same as above |

(Rows collapsed across identical target-risk results for readability; the full 24-row breakdown —
one row per (score, calibration method, target) combination, none omitted — is in the regenerated
`report/run_summary.json` / `report/REPORT.md`, gitignored, reproduce with the command above.)

## What this does and does not show

- The adapter → schema → `masstrust validate-split` → `calibrate` → `evaluate` pipeline runs end
  to end against real spectra, real external (`matchms`) scores, and real masstrust CLI output —
  not a toy fixture.
- At n=4/4, several (score, target) combinations abstain on everything (`abstain_all: true`) or
  land on 50%-coverage/50%-realized-risk with a [0.0, 1.0] bootstrap CI — both are the honest
  small-n result, not a bug.
- No Coverage@Risk-5% (or 10%, or 20%) number here should be read as a validated claim about
  masstrust's performance on cancer-relevant DNA-adduct annotation. See `FEASIBILITY.md` §0/§2.3
  for the pre-registered floor this dataset does not meet.
- Ground-truth-tier comparison is not implemented: all 8 queries share one tier
  (`reference_standard`). See `FEASIBILITY.md` §2.3.
- Colibactin is not represented anywhere in this dataset or this report (see `FEASIBILITY.md` §1.5
  and §2.1) — nothing here is evidence about colibactin.
