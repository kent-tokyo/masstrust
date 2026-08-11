# Feasibility report: cancer-related DNA-adductomics MS/MS selective-annotation benchmark

**Verdict, in one line: colibactin-specific candidate-ranking data is NO-GO. The best public
substitute found (general DNA-adductomics reference-standard data) is *also* NO-GO as a
benchmark — it fails this report's own pre-registered §0 floor and the brief's STOP D — but is
real enough to run once as an explicitly-labeled pipeline-verification preflight, never presented
as a Coverage@Risk result.**

Both verdicts are NO-GO under the current brief's own gate (§4/§27: "dataset数が少なすぎ、
calibration/test splitが成立しない" → STOP D applies to the 8-compound dataset just as much as
data-absence applies to colibactin). An earlier draft of this report mislabeled §2 "GO,
small-n" — that conflated §12's guidance on *how to report a benchmark that already cleared the
gate* with permission to relabel one that didn't. Corrected below.

This mirrors the precedent already set in this repo by
`benchmarks/selective_msms/PLAN.md` → `benchmarks/selective_msms_external/`: a pre-registered
stop condition was hit for the originally-requested scope (colibactin), a narrower but genuinely
real artifact was found instead, and the benchmark is named and scoped to match exactly what was
reconstructed — not what was originally hoped for.

---

## 0. Pre-registered minimum sample size (fixed *before* looking at what data exists)

masstrust's own `calibration::binomial::wilson_upper` (`crates/masstrust-core/src/calibration/binomial.rs`)
computes a one-sided Wilson upper confidence bound on risk from `(errors, accepted)`, using
`z = 1.645` at the 95% confidence level actually shipped (`z_for_confidence_level`). Solving that
same formula for the accepted-count needed to clear a target bound, with zero or one observed
error:

| target risk bound | accepted, 0 errors | accepted, ≤1 error |
|---|---|---|
| ≤ 10% | **≥ 25** (UB ≈ 0.098 at n=25) | **≥ 45** (UB ≈ 0.094 at n=45) |
| ≤ 5% | **≥ 52** (UB ≈ 0.049 at n=52) | **≥ 90** (UB ≈ 0.048 at n=90) |

(`z²/(n+z²)` for the 0-error case, solved directly; the ≤1-error rows were found by evaluating
`wilson_upper` at increasing `n` until it cleared the target — same function, not a re-derivation.)

This is the bar Phase A was checked against: **a headline Coverage@Risk number needs at least
~25–50 accepted queries** (more for a 5% claim), and per §11/§16 of the brief, calibration and
test must be **compound-disjoint**, so the dataset needs roughly double that many *distinct
compounds* with usable spectra to split fairly — call it a soft floor of **≥15–20 distinct
ground-truthed compounds** for a defensible headline. Anything below that is reportable only as a
small-n / pipeline-verification result, explicitly labeled as such (this is exactly what §12 of
the brief anticipates: "20 accepted, 1 error = 5%, don't over-read that").

---

## 1. Colibactin: what was searched, what was found, verdict NO-GO

Searched directly (WebSearch/WebFetch, first-party) and via three independent research agents,
each instructed to verify every accession/DOI itself rather than trust secondary summaries:
GNPS/MassIVE dataset search, MetaboLights, Metabolomics Workbench, PRIDE, and the data-availability
statements of the field's landmark structure papers.

### 1.1 Repositories

| repository | searched | found |
|---|---|---|
| GNPS / MassIVE | yes (web search + direct dataset fetches) | **one real, relevant, CC0 dataset**: `MSV000087101` — see 1.3 |
| MetaboLights | yes | no colibactin/precolibactin study found |
| Metabolomics Workbench | yes | no colibactin/precolibactin study found |
| PRIDE | not directly (proteomics-focused; MSV000087101's proteomics arm went to PRIDE as `PXD025088`, not relevant here) | — |
| Zenodo/Dryad (via paper citations) | yes | Dryad deposit found for the newest paper — see 1.4 |

### 1.2 Landmark papers — MS evidence is real, but lives in SI PDFs only

Verified directly (PMC full text where the publisher page 403'd — Science/ACS block plain
fetches):

| paper | DOI | rough tier | MS data public? |
|---|---|---|---|
| Wilson et al., *Science* 2019 | 10.1126/science.aar7785 | A/B border (¹³C-labeling + MS²/MS³ + synthetic-adduct standards) | No repo — SI PDF only (PMC6407708) |
| Xue et al., *Science* 2019 | 10.1126/science.aax2685 | A (synthetic colibactin, LC-MS co-injection) | No repo — SI PDF only (PMC6820679) |
| Carlson/Haslecker et al., *Science* 2025 | 10.1126/science.ady3571 | B (in-vivo ¹⁵N/¹³C labeling, MS+NMR) | **Yes — Dryad**, see 1.4 |
| Xue/Shine/Wang/Crawford/Herzon, *Biochemistry* 2018 | 10.1021/acs.biochem.8b01023 | B (MS²/MS³, ¹³C-auxotroph labeling) | No repo — SI only (PMC6997931) |
| Vizcaino & Crawford, *Nat Chem* 2015 | 10.1038/nchem.2221 | B (NMR+HRMS+¹³C-labeling) | No repo — SI figures only (PMC4499846) |
| Healy et al. (precolibactin 886), *Nat Chem* 2019 | 10.1038/s41557-019-0338-2 | A (LC/MS co-injection with synthetic standard) | No repo — "available from authors" (PMC6761996) |
| Zha/Wilson/Brotherton/Balskus, *ACS Chem Biol* 2016 | 10.1021/acschembio.6b00014 | not assessable | paywalled, no PMC mirror found |
| Li et al. ("macrocyclic colibactins"), *Nat Chem* 2019 | 10.1038/s41557-019-0317-7 | not assessable; structure is contested (Herzon 2020 critique + reply) | paywalled/unextractable |

**None of the seven assessable landmark papers deposit raw or processed MS/MS data in
GNPS/MassIVE/MetaboLights/Metabolomics Workbench/PRIDE.** The field runs on supplementary-PDF
peak tables and figures — not machine-readable, not bulk-downloadable, and not licensed for
redistribution as a derived candidate-ranking dataset (SI content is under the publisher's own
copyright, not CC-licensed).

### 1.3 The one real MassIVE dataset (`MSV000087101`) — real, but the wrong shape

Sadecki, Balboa, Lopez, Kedziora, Arthur, Hicks, "Evolution of Polymyxin Resistance Regulates
Colibactin Production in *Escherichia coli*," *ACS Chem Biol* 2021, 16(7):1243–1254, DOI
10.1021/acschembio.1c00322 (PMID 34232632). CC0 1.0, Q Exactive HF-X, PI Leslie Hicks
(UNC-Chapel Hill). Verified directly against the live dataset page and the paper's own Data
Availability statement (PMC8601121).

- **Real pks+ vs. pks− controls, identifiable by filename**: `NC101` (wild-type, pks+), `DPKS`
  (Δ*pks*, pks−), `MEDIA-BLANK`, and three independently-evolved polymyxin-resistant lineages
  (`3151`/`3152`/`3153`), 3 replicates each, 18 raw runs total.
- **But it is a targeted, single-analyte assay, not untargeted/candidate-ranking data.** The only
  processed artifact is `quant/20210301_PWS_metabolite_analysis.csv` (downloaded directly,
  2,368 bytes — see below), a single-component (`ClbP_prodrug`) peak-area report across all 18
  runs. No GNPS molecular-networking job is linked (the dataset page's GNPS section is an
  unrendered template placeholder; "Spectra: 0", "Dataset Reanalyses: none"). No multi-candidate,
  cosine-scored feature table exists anywhere in this dataset.
- The 18 `.mzML` files present (`ccms_peak/`, ~77 KB each) are far too small to be full-scan MS2
  sets — almost certainly narrow extracted-ion/SIM windows around one target, not untargeted data.
  Building a real candidate-ranking task from this would require re-picking features from the
  18 raw `.raw` files (238 MB) with an untargeted feature finder and manually curating a candidate
  pool against literature precolibactin masses — real, bounded engineering, but the result would
  still only be **presence/absence of one already-known analyte**, i.e. not a candidate-ranking
  retrieval task at all (masstrust's schema needs *multiple* candidates per query with a
  correctness label; this dataset gives one candidate, well below even n=18 after excluding blanks
  — under half our own pre-registered floor).

The downloaded quant CSV (real numbers, not reproduced here in full) shows `NC101` peak areas in
the 6.6–8.9M range, the three resistant lineages 20–48M (3–7× higher, matching the paper's
finding), and `DPKS`/`MEDIA-BLANK` all `NF` (not found) — internally consistent with the paper,
confirming the file is genuine.

### 1.4 The Dryad deposit (`Carlson et al. 2025`) — real repository deposit, wrong task shape

`datadryad.org/dataset/doi:10.5061/dryad.vmcvdnd5g`, verified directly. **5.68 GB**, six zip
archives of Thermo `.raw` files (Orbitrap Fusion/Lumos) plus a README — no processed peak lists,
no feature tables, no candidate annotations. This is **intact double-stranded DNA
oligonucleotide** (14-mer/25-mer) mass spectrometry — measuring the mass shift of a whole
duplex after exposure to colibactin-producing bacteria, to detect interstrand-crosslink (ICL)
formation — not small-molecule MS/MS spectral matching against a candidate-structure list. Their
own README names the processing software required: Thermo Protein Deconvolution, Xcalibur
FreeStyle/Qualbrowser, and Mongo Oligo Mass Calculator — specialized, largely Windows-only,
intact-oligonucleotide deconvolution tools, not anything in masstrust's candidate-ranking
pipeline shape. License is not explicitly stated on the Dryad page. Reprocessing this into a
candidate-ranking task is out of scope for a "minimal" real-data pass, and the task itself
(oligomer mass-shift detection) doesn't map onto masstrust's per-query multi-candidate-structure
schema without redefining what a "candidate" even means here.

### 1.5 Colibactin verdict

**NO-GO.** Zero public artifacts supply, for real colibactin-related MS/MS spectra, both (a) a
candidate ranking or a practical means to generate one, and (b) a ground-truth label at any tier
— let alone the ≥15–20-distinct-compound floor from §0. The two real repository artifacts found
(§1.3, §1.4) are both genuine and both the wrong shape for this task, not fabricatable into the
right shape without either reducing to a single-analyte presence/absence question (not a
retrieval task) or an unbounded intact-oligomer reprocessing effort. This is not a data-quality
problem to route around — it is a **complete absence of public candidate-annotation data for this
specific molecule class**, exactly the situation the brief's own STOP/REPORT gate anticipates.

**What would make it possible:** a wet-lab collaborator (or the Crawford/Herzon/Balskus labs
directly) sharing (i) the processed peak lists / MS² fragment tables that already exist in their
SI figures, in machine-readable form, or (ii) the raw LC-MS/MS files behind any of the papers in
§1.2 with permission to redistribute a derived candidate-ranking CSV. Either would clear the
data-existence bar immediately — the field's science is not the blocker, its data-sharing practice
is.

**Best alternative already identified:** the general DNA-adductomics database below — real,
public, licensed for reuse, but explicitly does not include colibactin (confirmed directly, §2.1).

---

## 2. General cancer-relevant DNA-adductomics: GO, at small-n scale

### 2.1 The dataset: nexs-metabolomics "DNA adduct database"

La Barbera et al., "A Comprehensive Database for DNA Adductomics," *Frontiers in Chemistry* 2022
(PMC9184683), DOI-backed, **CC-BY 4.0**, hosted at
`gitlab.com/nexs-metabolomics/projects/dna_adductomics_database` (actively maintained: 29 commits,
last substantive update well after the 2022 paper — the live database has grown past the
published snapshot).

Downloaded and parsed directly (not taken from the paper's abstract):

- **`public/Database for MS.xlsx`** (222,429 bytes, downloaded and opened with `openpyxl`):
  **717** compound records, columns `Name`, `Short name`, `Formula`, `Monoisotopic mass`,
  `Charged monoisotopic mass`, `Charged monoisotopic mass -dR` (deoxyribose-loss mass), `Source`
  (genotoxicant class: ROS, RNS, alkylating agents, PAHs, HAAs, N-nitroso compounds, mycotoxins,
  pyrrolizidine alkaloids, furans, acrylamide, lipid peroxidation, estrogens, halogenation,
  aristolochic acid, alkenylbenzenes), `Adduct`, `Reference` (a DOI per compound), `SMILES`,
  `InChI`, `InChIKey`, `IUPAC Name`. Real structures also ship as `.mol`/`.SDF` files.
- **`public/experimental.html`**: an R/DT interactive table; its embedded JSON (extracted directly
  with a bracket-matched parse, not guessed) is a **1,506-row long-format fragment table** — one
  row per (compound, collision energy, fragment ion) — covering **8 distinct compounds** with
  real, instrument-acquired MS/MS spectra: **5-Methyl-dC, N6-Methyl-dA, O6-Methyl-dG, 8-Oxo-dG,
  Acrolein-1I-dG (Acr-1I-dG), Malondialdehyde-1-dG (M1-dG), N6-(2-Hydroxy-ethyl)-dA, and
  8-(N′-Aminobiphenyl)-dG (8-ABP-dG)**, at CE 20/40/60/80 eV, ESI+, on a Waters Vion IM-QTOF
  (H-Class Acquity UHPLC). (The 2022 paper describes 15 reference standards; only 8 currently
  carry parsed experimental fragment rows in the live repo — the database has evolved since
  publication. Treat 8 as the operative number, not 15.)
- **`public/predicted.html`**: the same structure, **582 distinct compounds** with real CFM-ID
  in-silico predicted fragment spectra (23,391 fragment rows) — this is the real candidate
  reference library that makes a retrieval task possible: for any query, mass-filter the 582
  candidates against the query's own precursor mass, and every candidate has a genuine predicted
  spectrum to score against, not a placeholder.

### 2.2 What this dataset does support

- **Ground truth**: each of the 8 experimental compounds is an authentic reference standard,
  directly infused/injected and its MS/MS spectrum acquired — unambiguous identity of what was in
  the vial. This is Tier-A-*equivalent* for "this spectrum's true structure is X" (not the paper's
  literal Tier A, which requires co-injection into a biological matrix — these are pure-standard
  spectra, and the report below labels them precisely as `reference_standard`, not
  `biological_co_injection`).
- **Cancer relevance, without colibactin**: **8-(N′-Aminobiphenyl)-dG** derives from
  4-aminobiphenyl, an IARC Group 1 human carcinogen (bladder cancer) — a genuine cancer-adduct
  case, and the best available real substitute for a "killer use case" now that colibactin itself
  is confirmed unavailable (§1.5).
- **Realistic candidate pool, not fabricated decoys**: the other ~581 candidates are real,
  distinct, database-documented molecules spanning 16 genotoxicant classes — not synthetic
  placeholders. Mass-filtering (±tolerance on precursor mass, mirroring the mass-filtered
  candidate pool convention already used by `benchmarks/massspecgym/`) yields a realistic,
  same-formula-confusable pool per query, satisfying §7's "don't make it trivial" requirement
  without inventing anything.
- **License**: CC-BY 4.0 — redistributable with attribution, unlike the colibactin SI PDFs.
- **A real, non-fabricated candidate-pool provenance axis** (verified directly against the
  downloaded xlsx, not assumed from the paper): of the 717 compound records, **279 carry a
  literature-reference DOI** in the `Reference` column (`has_reference` — literature-confirmed
  identity), and **438 do not** (`no_reference` — a suspected/theoretical combinatorial entry;
  the 2022 paper's "303 suspected" figure, grown since publication). This is a genuine tier axis
  for the *candidate pool*, usable as `candidate_origin` provenance in the exported CSV — but see
  §2.3 for why it does **not** rescue §15's tier-comparison requirement.

### 2.3 Why this fails the brief's own gate (STOP D)

**8 distinct compounds is below the ≥15–20 floor from §0.** A compound-disjoint calibration/test
split (required by §16/§11 of the brief) leaves ~4 compounds per side; even with 4 CEs each,
that's on the order of 16 queries per split — short of the ≥25-accepted floor for even a ≤10%-risk
claim, let alone ≤5%. §27 STOP D is explicit: "dataset数が少なすぎ、calibration/test splitが
成立しない" — that is exactly this dataset's condition. §12's "report 5/10/20%, don't over-read
20-accepted-1-error" guidance governs how to *present a benchmark that already cleared the gate*;
it is not a license to call an 8-compound dataset a benchmark in the first place.

**§15 (ground-truth-tier risk-coverage comparison, called out as "特に重要") is structurally
unimplementable here, not just under-powered.** All 8 experimental compounds are the same tier
(`reference_standard` — an authentic standard, directly measured, unambiguous identity). There is
no Tier A vs. B vs. C vs. D *query* population to compare risk-coverage curves across. The
279-vs-438 `has_reference` split found in §2.2 is a real axis, but it lives on the **candidate
pool**, not on the 8 ground-truthed queries — it cannot substitute for §15's requirement, which is
about how confidently *queries* are labeled, not how well-documented *candidates* are.

### 2.4 Verdict

**NO-GO as a benchmark, under this report's own §0 floor and the brief's STOP D.** Per §4/§27:
"無理にbenchmarkを作らないでください" — no Coverage@Risk headline, no risk-coverage curve
presented as a result, and no claim that masstrust has been benchmarked on cancer-relevant
DNA-adductomics data.

(The one integrity check worth recording here: `is_correct` is assigned by exact InChIKey match,
which would understate accuracy if a "wrong" top-1 pick were actually a stereoisomer/duplicate
entry of the true compound. Checked directly for all 4 wrong top-1 picks in the local preflight
run — every one has a different 14-character 2D-skeleton block from the true compound, so this is
not a labeling artifact. This check, and the full preflight run it comes from, will be committed
as `PREFLIGHT_REPORT.md`'s "Label-conflation check" section in the follow-up PR described in
`README.md` — not part of this PR.)

What the dataset *is* real enough for: one explicitly-labeled **pipeline-verification preflight**
— real spectra, a real external similarity scorer, real masstrust CLI calls, run once end-to-end
to prove the adapter → schema → calibrate → evaluate shape actually works on real data (the
brief's §18 "toy fixtureだけでは成功扱いにしないこと" bar), with every output banner-labeled
`run_kind=preflight`, exactly mirroring `benchmarks/massspecgym/`'s own precedent for a run that
is real but not statistically representative. It must never be summarized as a Coverage@Risk-N%
achievement in any README, changelog, or report.

---

## 3. Candidate-ranking adapter tool

Investigated MetFrag (headless CLI, LGPL, ingests a local candidate CSV — confirmed via GitHub
releases + docs) and SIRIUS+CSI:FingerID (AGPL, but CSI:FingerID's fingerprint step needs a
cloud-service login even with a local candidate DB — disqualifying for a fully offline pipeline).
MetFrag was the general recommendation for a *from-scratch* hand-curated candidate list.

**For this dataset specifically, a better fit exists and was chosen instead: `matchms`**
(Apache-2.0, pure-Python + numpy/scipy/numba, pip-installable, no JVM). The DNA-adductomics
database already ships **real in-silico predicted spectra (CFM-ID) for all 582 candidates** —
running MetFrag's own theoretical fragmenter on top would be redundant re-derivation of data that
already exists. `matchms` computes standard cosine / modified-cosine spectral similarity directly
between the real experimental query spectrum and each mass-filtered candidate's real CFM-ID
spectrum — this *is* "external annotation engine → candidate ranking" in the sense §4/§5 of the
brief require, using an established, peer-reviewed, widely-used (GNPS ecosystem) similarity
scorer as the external tool, without masstrust (or this benchmark harness) implementing its own
fragmentation logic. No Java runtime is available on this machine without a new install; `matchms`
avoids that dependency entirely. MetFrag remains the documented fallback if a future
hand-curated-candidate-list dataset (e.g. a future colibactin dataset, should one become
available) doesn't ship its own predicted spectra.

---

## 4. What this benchmark is not, and forbidden framings

Per the repo's own established convention (`benchmarks/selective_msms_external/README.md`), stated
explicitly rather than discovered later:

- This is **not** a colibactin benchmark. Colibactin is confirmed absent from this dataset
  (§1.5, §2.1). Do not title or describe any output as a colibactin result.
- This is **not** a statistically validated Coverage@Risk-5% (or 10%) result. n=8 compounds is a
  pipeline-verification/small-n demonstration, not a benchmark headline, per §0/§2.3.
- This is **not** a biological-sample or co-injection confirmation. The 8 "ground truth" spectra
  are pure reference-standard identities, not biological-matrix annotations.
- Do not claim: "masstrust detects colibactin", "masstrust proves DNA damage", "masstrust
  diagnoses cancer", "carcinogen exposure confirmed". Use: "candidate DNA-adduct annotation",
  "annotation accepted/abstained under a calibrated policy", "putative", per §14/§24 of the brief.
- **This directory does not contain a colibactin benchmark, and it does not contain a general
  DNA-adductomics benchmark either.** §19 of the brief gates generalization to "general DNA
  adductomics" on a colibactin benchmark existing first ("Colibactin benchmarkが成立したら…ここで
  初めて`DNA-adductomics selective annotation`として一般化してください"). §1.5 established that
  gate is not met. Nothing here should be described as fulfilling §19's Phase 2, and no output
  should be titled or summarized as a "DNA-adductomics selective-annotation benchmark result."
  What exists is a design (README.md) and, at most, one preflight run against real data that
  neither gate authorizes calling a benchmark.

## 5. Recommended path forward

1. **PR 1 (this report + `README.md`, no pipeline code)**: feasibility + protocol design only,
   per §26 of the brief. This is where this phase stops without an explicit go-ahead to write the
   scripts under §2.4's preflight-only framing.
2. Do not attempt a colibactin-specific Phase 1 until a wet-lab collaborator (§1.5) or a future
   public deposit changes §1's verdict — that remains the actual, brief-mandated Phase 1 gate for
   any generalization claim.
3. If a real-data pipeline-verification preflight is still wanted despite §2.4's NO-GO (matching
   the brief's own §18 "at least one real-data end-to-end run, not a toy fixture" instinct, the
   same way `benchmarks/massspecgym/`'s preflight exists without being a benchmark), build it as
   `run_kind=preflight` only, with this file's STOP-D verdict linked from its README, never
   restated as a cleared benchmark.
4. Re-run §0's minimum-n check, and re-check whether §15's tier axis is now populated, against any
   future candidate dataset before calling anything here a benchmark.
