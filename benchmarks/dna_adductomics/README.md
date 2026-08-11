# masstrust cancer-DNA-adductomics selective-annotation design

**Status: design + one preflight run, not a benchmark.** See `FEASIBILITY.md` for the full
evidence trail. In one line: colibactin-specific candidate-ranking data does not exist publicly
(NO-GO), and the best available general-DNA-adductomics substitute (8 real reference-standard
compounds) fails this project's own pre-registered minimum-n floor (also NO-GO, as a benchmark —
see `FEASIBILITY.md` §0/§2.3/§2.4, STOP D). Nothing in this directory should be cited as a
DNA-adductomics selective-annotation *result*. What follows is (a) the protocol this benchmark
*would* run once real data clears the gate, written now so it's ready to execute without
re-deriving the design later, and (b) `scripts/` for the one thing current data does support: a
single real-data **pipeline-verification preflight**, in the same sense
`benchmarks/massspecgym/`'s preflight exists — real spectra, real external scoring, real
masstrust CLI calls, explicitly not a statistically representative result.

## Scope boundary (unchanged from the rest of this repo)

```text
MS/MS spectrum (real, public)
    ↓
external annotation / retrieval engine   (matchms spectral similarity — see FEASIBILITY.md §3)
    ↓
candidate ranking
    ↓
masstrust schema (query_id, candidate_id, rank, score, is_correct, + provenance)
    ↓
masstrust confidence scoring / calibration / evaluation
    ↓
trust / abstain
```

masstrust does not perform spectral matching, fragmentation simulation, or DNA-adduct/carcinogen
detection. It decides whether to trust or abstain on a candidate annotation produced upstream.
Forbidden framings for anything produced here: "masstrust detects colibactin", "masstrust proves
DNA damage", "masstrust diagnoses cancer", "carcinogen exposure confirmed", "DNA-adductomics
selective-annotation benchmark" (§19's gate is not met — see `FEASIBILITY.md` §4). Allowed:
"candidate DNA-adduct annotation", "annotation accepted/abstained under a calibrated policy",
"putative".

## The protocol (ready to execute once a dataset clears `FEASIBILITY.md` §0)

1. **Data.** Real, public MS/MS spectra with (a) a known true structure per spectrum and (b) a
   ground-truth-tier label (`reference_standard` / `synthetic_standard_co_injection` /
   `isotope_labeled` / `biological_control` / `putative_mass_only` — see the tier rubric in
   `FEASIBILITY.md` §0 and the brief's own Tier A–D definitions). At least ~15–20 distinct
   compounds, so a compound-disjoint calibration/test split leaves enough queries on each side to
   clear the Wilson-bound floor derived in `FEASIBILITY.md` §0.
2. **Candidate pool.** Mass-filtered (±tolerance on precursor mass) subset of a real, documented
   compound library — never fewer than the true match plus several same-formula/same-nucleobase
   real candidates. Every candidate row carries `candidate_origin` (`literature_confirmed` /
   `suspected_theoretical` / `synthetic_decoy`) so a reader can tell real structural confusion
   from an inserted decoy — see `FEASIBILITY.md` §2.2 for why this matters and what's actually
   available.
3. **External scoring.** `matchms` cosine / modified-cosine similarity between the real query
   spectrum and each candidate's own spectrum (experimental-vs-experimental where available,
   experimental-vs-in-silico-predicted otherwise — labeled which, never conflated). This is the
   external "annotation engine" in the adapter-boundary sense; masstrust never re-implements it.
4. **masstrust schema export**, following the established convention in
   `benchmarks/selective_msms_external/scripts/convert_to_masstrust_csv.py` and
   `benchmarks/massspecgym/README.md`'s "Output schema" table: the four required columns
   (`query_id,candidate_id,rank,score,is_correct`) plus provenance columns this benchmark adds —
   `ground_truth_tier`, `evidence_kind`, `reference_doi`, `candidate_origin`, `genotoxicant_class`,
   `nucleobase`, `collision_energy`, `instrument`, `precursor_mz`. `masstrust-core`'s CSV reader
   already ignores unrecognized columns and tolerates missing optional ones — verified directly
   against `crates/masstrust-core/src/io.rs`'s own test suite
   (`test_read_candidates_optional_columns_absent`); no core change needed to carry these.
5. **Split.** Compound-disjoint calibration/test (never spectrum-level random — §16 of the brief
   is explicit that a replicate-level random split must not be a headline). Verified with
   `masstrust validate-split`.
6. **Baseline first.** `masstrust compare` across `score-gap`, `score-ratio`, `topk-gap`,
   `candidate-count` (matchms similarity is a bare score, not a calibrated probability — do not
   feed it to `max-prob`/`margin`/`entropy`/`effective-k` as though it were one; those need a
   genuine posterior, which this pipeline does not produce, matching the same caveat already
   documented in `benchmarks/massspecgym/README.md`).
7. **Ground-truth-tier analysis** (brief §15, "特に重要"): risk-coverage curves computed
   separately for (Tier A+B only) vs. (Tier A+B+C) vs. (all tiers) query populations — **requires
   the query set to span more than one tier**, which the current 8-compound dataset does not (see
   `FEASIBILITY.md` §2.3). Not implementable until a dataset with mixed-tier queries exists.
8. **Metrics.** Top-1 accuracy, AURC, E-AURC, risk-coverage curve, Coverage@Risk at 5/10/20%
   (side by side, per §12 — never just 5% when n is small), realized risk, accepted/abstained
   counts, bootstrap CI (`masstrust curve --bootstrap` / `masstrust evaluate --bootstrap`).
9. **Hard-negative analysis** (brief §14): categorize abstained queries by failure class —
   isomer ambiguity, same-formula ambiguity, weak fragmentation, low-intensity spectrum,
   conflicting evidence — report the distribution, not just aggregate coverage/risk.
10. **Manifest.** Following `benchmarks/massspecgym/`'s convention: dataset accession/commit/hash,
    `masstrust` commit SHA, dirty-tree flag, dependency versions (`requirements.lock.txt`),
    `matchms` version, candidate-pool provenance, split strategy, seed, `run_kind`
    (`preflight`/`benchmark`) — `generate_report.py` must refuse to present `preflight` output as
    a benchmark result, exactly like `benchmarks/massspecgym/generate_report.py` already does.

## What the preflight covers

`scripts/` implements only the preflight — real data, `run_kind=preflight` stamped everywhere,
banner-labeled in every generated report. It exercises steps 1–6, 8, and 10 above end to end
against the 8-compound / 582-candidate dataset from `FEASIBILITY.md` §2.1, to prove the adapter →
schema → calibrate → evaluate shape works on real spectra (the brief's §18 "not a toy fixture"
bar) — it does not, and cannot, satisfy steps 7 or the compound-disjoint-split power needed for
step 8's numbers to mean anything at scale. The committed `PREFLIGHT_REPORT.md` carries the
explicit small-n disclaimer on every number — that file, not a re-run's local `report/REPORT.md`
(gitignored, regenerated fresh each run), is the point-in-time record.

`scripts/selftest.py` is the one runnable check that doesn't need the real dataset: it builds a
tiny synthetic `database.xlsx`/`experimental.html`/`predicted.html` triple with the same column
layout as the real files and runs the full `export_candidates.py → validate_data.py →
run_benchmark.py → generate_report.py` chain against it, asserting on schema (lowercase
`true`/`false`, non-trivial candidate pools), split disjointness, and report generation — pipeline
*mechanics*, not scientific correctness on real data (that's what `PREFLIGHT_REPORT.md` is for).
Run it before ever pointing the pipeline at the real dataset, or in any environment without
network access:

```bash
cd benchmarks/dna_adductomics
python3 scripts/selftest.py
```

Reproduce against the real dataset:

```bash
cd benchmarks/dna_adductomics
pip install -r requirements.txt

python scripts/prepare_data.py --out-dir ./data          # downloads + checksums the CC-BY 4.0 dataset
python scripts/export_candidates.py --data-dir ./data     # matchms scoring → masstrust CSV
python scripts/validate_data.py --data-dir ./data        # schema + compound-disjoint leakage check
python scripts/run_benchmark.py --data-dir ./data --out-dir ./report
python scripts/generate_report.py --data-dir ./data --report-dir ./report
```

## Licensing / attribution

La Barbera G et al., "A Comprehensive Database for DNA Adductomics," *Frontiers in Chemistry*
2022, database at `gitlab.com/nexs-metabolomics/projects/dna_adductomics_database`, **CC-BY 4.0**.
Cite this paper and the repository for any reuse. `matchms` (Apache-2.0): Huber F et al.,
*Journal of Open Source Software*, used here as the external spectral-similarity scorer.
