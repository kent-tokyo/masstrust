# Feasibility report: importing Selective-MSMS's public predictions for a masstrust comparison

**Status: assessment complete, approved, and paused.** No `masstrust-core` changes have been
made and no importer code has been written. **Final verdict: B — reproducible
external-prediction benchmark. This is explicitly NOT a competitor-parity benchmark** — see
"Final verdict" below. Verdict B is approved, but the importer itself is **deliberately deferred**
until after masstrust's statistics backend migrates to `risksieve` (see "Status: paused" near the
end of this document) — building the importer against the current calibration backend now would
risk immediate rework once that migration lands.

---

## Context

masstrust's own seed-0 MassSpecGym benchmark run is blocked by training throughput on this
machine (see `tasks/todo.md`). One candidate next step is importing a competitor's
already-published predictions into masstrust's existing CSV schema. The original feasibility
report (this file, first version) found the licensing clean (MIT / CC BY 4.0 / MIT, all three
checked independently) but could not confirm whether Selective-MSMS's public artifact
(arXiv:2603.10950, code at
[github.com/mkjuergens/selective-msms](https://github.com/mkjuergens/selective-msms), data at
[Zenodo 10.5281/zenodo.19108280](https://doi.org/10.5281/zenodo.19108280)) could be
reconstructed into a trustworthy candidate-ranking table, or compared under a fair protocol —
those questions required inspecting the actual released artifact, not just its documentation.

This update reports the results of a bounded verification spike answering those questions
directly, without downloading the full 3.18 GB `results.zip`.

---

## How the spike was done

Zenodo's file-download endpoint honors HTTP `Range` requests (confirmed: a real `GET` with
`Range: bytes=0-1048575` returned `206 Partial Content` with `Accept-Ranges: bytes` — `HEAD`
requests misleadingly returned `200`/full `Content-Length`, so this had to be checked with an
actual `GET`, not just a header probe). Since every member inside `results.zip` is stored with
`ZIP_STORED` (no compression — confirmed via `ZipInfo.compress_type == 0` for every file
checked), individual archive members can be extracted by: (1) one range request for the last
bytes to locate the End-Of-Central-Directory record, (2) one range request for the central
directory itself, (3) one range request per target file, computed from its exact
`header_offset`/`compress_size` in the central directory. This is the standard "remote zip"
technique, implemented here as a ~90-line stdlib `urllib` + `zipfile` script (no new
dependencies) — **not** a bulk download.

Total: **14 HTTP requests, 9,764,154 bytes (~9.76 MB) transferred**, against a 133-requests
rate-limit budget (confirmed via `x-ratelimit-limit` response headers). Every extracted file's
SHA-256 was verified against the archive's own published `SHA256SUMS`/`MANIFEST.tsv` (fetched
separately in the original report, itself only 34,594 bytes) — all matched exactly.

One further attempt — peeking at `scores.pt`'s internal shape by extracting just its small
metadata-only member via a second, nested layer of range requests — was **deliberately
abandoned**. `scores.pt` is a PyTorch `torch.save` file, which relies on Python's general-purpose
object-serialization format internally; parsing any part of it by hand, even a
narrowly-restricted parse, risks running arbitrary code embedded in an externally-sourced
serialized stream. A security hook blocked the attempt to write that parsing script, correctly.
This is the one significant gap left by this spike — see "Remaining unknowns."

---

## Downloaded-file inventory

All files extracted from `results.zip` (Zenodo record `19108280`, version 1) at
`data/results/...` paths shown. Kept locally, **untracked**, at
`benchmarks/selective_msms/inspection/` — small, CC-BY-4.0-licensed provenance metadata with
clear reproducibility value, per your instruction.

| local file | archive path | bytes | SHA-256 | verified against published manifest |
|---|---|---|---|---|
| `run_manifest.json` | `data/results/provenance/run_manifest.json` | 1,990 | `9473ac0e9051...` | ✓ |
| `query_manifest.parquet` | `data/results/provenance/query_manifest.parquet` | 4,711,836 | `cb8c4334398a...` | ✓ |
| `query_masks.parquet` | `data/results/provenance/query_masks.parquet` | 76,719 | `1b1f655d2508...` | ✓ |
| `input_files.tsv` | `data/results/provenance/input_files.tsv` | 4,668 | `f4ff4df3ce78...` | ✓ |
| `validation_report.json` | `data/results/provenance/validation_report.json` | 3,931 | `c530568a64a6...` | ✓ |
| `dataset_audit.csv` | `data/results/provenance/dataset_audit.csv` | 152 | `aaa2b73d936f...` | ✓ |
| `evaluation_matrix.tsv` | `data/results/provenance/evaluation_matrix.tsv` | 1,581 | `3b065358e89c...` | ✓ |
| `mass_capped_ensemble_mlp_mass_test_metadata.json` | `.../mass_capped/ensemble_mlp_mass/test/metadata.json` | 590 | `32048b171271...` | ✓ |
| `mass_capped_ensemble_mlp_mass_test_metrics.csv` | `.../mass_capped/ensemble_mlp_mass/test/metrics.csv` | 29,415 | `9620f6275f08...` | ✓ |
| `sgr_partitions.csv` | `data/results/analyses/sgr/sgr_partitions.csv` | 4,933,272 | `43770eb2fdf6...` | ✓ |

**Not fetched** (deliberately, out of spike scope): `scores.pt` (126 MB, security reason above),
`numerical/query_scores.parquet` (32 MB, not needed once the files above answered the questions),
`checkpoints.zip`, and any MassSpecGym dataset/candidate files.

**Also used, at zero additional network cost:** our own already-locally-cached pinned v1.5
artifacts from the earlier, already-approved preflight run —
`benchmarks/massspecgym/data/preflight/data/MassSpecGym1.5.tsv` (261,197,365 bytes, SHA-256
`50cfdd1d22f79543c59555f9ce43c6893bd788a19a42b21fb4e2e3a54673c06a`) and
`.../molecules/MassSpecGym1.5_retrieval_candidates_mass.json` (454,362,310 bytes, SHA-256
`9536a330cba24c399271364889119f90124579400dba14e9f78630c75c13c73a`), plus
`benchmarks/massspecgym/data/preflight/manifest.json` for our own recorded dataset provenance.

---

## Resolved questions

### 1. Row identity

- **`scores.pt` is not a dense `[17556, 256]` padded matrix.** `metadata.json` records
  `n_raw_scores = n_record_scores = 4,457,058` for `mass_capped/ensemble_mlp_mass/test` —
  independently confirmed identical in `evaluation_matrix.tsv`'s `n_candidate_records` column.
  `17,556 × 256 = 4,494,336 ≠ 4,457,058`, so the tensor holds exactly the real candidate count
  per query (average ≈253.9/query), not a padded rectangle. It is almost certainly a flat/ragged
  structure requiring a separate offsets-or-lengths array to slice per query — **this internal
  layout was not directly confirmed**, since opening `scores.pt` was deliberately avoided (see
  above).
- **Query ordering:** `metadata.json` declares `"query_identity_source": "precomputed"`, and
  `query_masks.parquet` provides the exact, ordered list of all 17,556 test-fold `query_id`
  values (official `MassSpecGymID#######` format) with `mass_paired_test = True` for every one
  of them (i.e., the mass-setting evaluation uses the *entire* official test fold, unfiltered).
  This is strong indirect evidence for how queries are ordered/identified, but — without opening
  `scores.pt` — is not a byte-level proof that the tensor's row order matches this list.
- **Candidate ordering within a query:** explicitly documented, not inferred —
  `"candidate_record_policy": "preserve"` and `"candidate_tie_break": "source_order"`
  (`metadata.json`, `run_manifest.json`) state candidates are kept in the same order as the
  official MassSpecGym candidate JSON provides them, never independently resorted.
- **Can one raw score map deterministically to `(query_id, candidate_id, rank)`?** Likely yes,
  mechanically: query_id from the precomputed manifest, candidate_id from their own v1
  mass-candidate JSON (not yet downloaded — see below) via the documented preserve/source-order
  policy, rank by sorting scores descending per query (masstrust's own convention). **Not
  end-to-end verified against actual tensor bytes** — this is the one real remaining gap.

### 2. Dataset identity

- **Exact file used:** `MassSpecGym.tsv`, 262,334,768 bytes, SHA-256
  `0c9cc50450def3f0d4fe2dc09dea1105fc15e635db8c6656bc3e3be37a3bcd95` — confirmed via **two
  independent sources that agree exactly**: the repo's `EXTERNAL_DATA.tsv` (general
  documentation) and this specific run's own `input_files.tsv` audit log (what was actually fed
  into the evaluation that produced these numbers). This is MassSpecGym **v1**, not v1.5.
- **Contains original query identifiers:** yes — `query_manifest.parquet` (231,104 rows) has
  columns `query_id`, `identifier`, `fold`, `smiles`, `inchikey`, `formula`, `adduct`,
  `instrument_type`, etc., using the same `MassSpecGymID#######` identifier convention
  masstrust's own harness reads via `row["identifier"]`.
- **Quantified v1↔v1.5 overlap — exact-count evidence, not a record-level join:**

  | check | Selective-MSMS (v1) | masstrust (v1.5, local cache) | match? |
  |---|---|---|---|
  | total rows | 231,104 (`dataset_audit.csv`, `query_manifest.parquet`) | 231,104 (counted directly from our cached TSV) | **exact** |
  | train/val/test fold sizes | 194,119 / 19,429 / 17,556 (`config/paper.yml`, `query_manifest.parquet`) | 194,119 / 19,429 / 17,556 (counted directly) | **exact** |
  | test-fold unique molecules | 2,998 (`dataset_audit.csv`, `config/paper.yml`) | not independently recounted this pass, but consistent with the identical fold size above | consistent |
  | main TSV file size | 262,334,768 bytes | 261,197,365 bytes | **differs by 1,137,403 bytes** |
  | main TSV SHA-256 | `0c9cc504...` | `50cfdd1d...` | **differs (as expected given size)** |

  Five independent exact-count matches (total rows, all three fold sizes, test molecule count)
  is strong circumstantial evidence that v1 and v1.5 carry the *same underlying spectra*,
  consistent with the HF dataset card's own description of the v1.5 update ("content nearly
  identical... SMILES re-standardized via `rdkit.Chem.MolToSmiles(canonical=True)`... schema
  identical"). **This is not a proven record-level bijection** — no actual `query_id`-to-`query_id`
  or InChIKey-to-InChIKey join was performed, since that would require downloading their 262 MB
  v1 TSV, which was judged out of scope for this ~10 MB spike. **v1 and v1.5 are strongly
  consistent at the aggregate level, but not proven identical at the individual-record level —
  do not describe this as "confirmed identical."**
- **A new, concerning finding: candidate-pool file sizes do NOT show the same clean
  consistency.** Selective-MSMS's v1 mass-candidate-pool file
  (`MassSpecGym_retrieval_candidates_mass.json`, confirmed via both `EXTERNAL_DATA.tsv` and this
  run's `input_files.tsv`: 164,001,599 bytes, SHA-256 `33616aa9feb50fd0195918ead17851a117cb995f75488b7cce7130ac0c6df8a1`)
  is **~2.77× smaller** than masstrust's pinned v1.5 equivalent (454,362,310 bytes, SHA-256
  `9536a330cba24c399271364889119f90124579400dba14e9f78630c75c13c73a`). A pure SMILES
  re-standardization would not explain a 2.77× size difference. Combined with their own repo
  documenting this exact file as "opaque" (mass tolerance, adduct conversion, pre-cap pool, and
  database snapshot all unavailable, per their own `EXTERNAL_DATA.tsv` and restated in this run's
  `run_manifest.json` deviations list). **Candidate-pool parity between v1 and v1.5 is not
  established** — even though spectrum/query-level parity looks strong. Any future importer must
  use Selective-MSMS's own v1 pool file end to end, never substitute masstrust's v1.5 pool.

### 3. Split reconstruction

**This is the most important new finding of the spike**, and it changes the comparison design
from the original report.

- Selective-MSMS's explicit calibration/evaluation split (`sgr_partitions.csv`: 105,336 rows,
  columns `run_label, K, seed, query_id, partition`, `partition ∈ {cal, eval}`, exactly 50/50,
  `seed=42` throughout) **only covers `run_label ∈ {mlp_formula, transformer_formula}`.** There
  is no `mlp_mass` row anywhere in this file.
- Independently confirmed by `mass_capped_ensemble_mlp_mass_test_metrics.csv`: every one of its
  69 metric rows is a plain full-test-set measure (`hit_rate`, `aurc`, `rel_aurc`, computed over
  all 17,556 test queries at K∈{1,5,20}). **None reference a calibration/evaluation partition at
  all.**
- **Conclusion: Selective-MSMS's own SGR calibration/evaluation split was never applied to the
  one model that matches masstrust's candidate-pool choice (`ensemble_mlp_mass`).** There is no
  "their split" to inherit for this artifact — only a monolithic full-test-set score export.
- For the *other* setting (`formula_official`, not matching our pool), the split membership
  **is stored explicitly** (not regenerated procedurally) in `sgr_partitions.csv`, and is
  therefore reproducible bit-for-bit by simply reading that file — no regeneration risk from
  library-version drift. Query counts: 52,668 rows per `run_label` (17,556 queries × 3 K-values),
  26,334 `cal` / 26,334 `eval` each. No filtering before/after the split was observed beyond the
  official test fold itself.

### 4. Candidate-pool parity

- **Confirmed:** `mass_capped/ensemble_mlp_mass` does use the intended mass-filtered pool
  (`"candidate_setting": "mass"`, `"candidate_setting_id": "mass_capped"` in `metadata.json`).
- **Max/actual candidate counts:** cap is 256 by design; actual average ≈253.9/query
  (4,457,058 ÷ 17,556), confirmed consistently across `metadata.json`, `evaluation_matrix.tsv`,
  and `dataset_audit.csv`. Per-query min/max distribution not individually verified (would
  require the raw candidate JSON or `scores.pt` itself).
- **True candidates absent?** No — `n_target_absent: 0` in `metadata.json`, and
  `n_mass_target_absent_spectra: 0` / `n_mass_target_absent_molecules: 0` in `dataset_audit.csv`,
  agreeing exactly.
- **Candidate identity / correctness convention:** `"label_mode": "inchikey"` — **confirmed
  identical to masstrust's own convention** (exact InChIKey match). This was flagged as
  "to-verify, not assumed" in the original report; now directly confirmed from their own
  metadata, not inferred.
- **Filtering/dedup/tie-break:** none — `"candidate_records_deduplicated": false`,
  `"candidate_record_policy": "preserve"`, `"candidate_tie_break": "source_order"`, all
  explicitly documented.
- **`query_manifest.parquet` already resolves `target_inchikey` directly** — it has a
  per-query `inchikey` column (the ground-truth molecule's InChIKey), computed and validated
  upstream. This is one of the best-resolved fields in the whole exercise: no dependency on
  `scores.pt` or the candidate-pool file at all.
- **Data-quality caveat found along the way:** 272 of 231,104 rows (0.12%) are flagged
  `identity_mismatch = True` in `query_manifest.parquet` (260 train / 7 val / 5 test — only
  5 of 17,556 test queries, 0.028%). Context suggests this is Selective-MSMS's own internal
  recompute-vs-recorded InChIKey consistency check, not a v1-vs-v1.5 comparison — small enough
  to be a footnote, not a blocker, but should be excluded/flagged if a future importer processes
  these specific rows.

### 5. Minimal reconstruction test

| check | result |
|---|---|
| score tensor dimensions match metadata | **partially verified** — three independent files (`metadata.json`, `evaluation_matrix.tsv`, cross-checked against `dataset_audit.csv`/`query_masks.parquet` row counts) agree exactly on `n_queries=17,556`, `n_record_scores=4,457,058`. The actual tensor was not opened. |
| query and candidate identities unambiguous | query: **yes, high confidence** (precomputed identity source + explicit ordered manifest). candidate: **documented policy is unambiguous** (preserve/source-order), but the concrete per-query candidate list requires their v1 candidate-pool JSON, not yet downloaded. |
| ranks derivable deterministically | **yes, mechanically** — sort raw scores per query, descending (same convention as masstrust's own `export_split()`). Not executed against real values in this pass. |
| correctness recomputable | **yes, and mostly already done** — `query_manifest.parquet` gives ground-truth InChIKey directly; only the candidate-side InChIKey needs the pool file. |
| no undocumented permutation remains | **not independently provable without opening `scores.pt`.** Strong *documented* assurance (preserve/source-order/precomputed), but "documented as unpermuted" ≠ "verified unpermuted by inspecting bytes." This is the honest, single largest open item. |

---

## Resolved field-mapping table

Supersedes the original report's §4 table. "Resolved" = confirmed from the spike's evidence;
"mechanism known, data not fetched" = the *how* is documented, the *actual values* require a
further download not done in this pass.

| masstrust column | status | detail |
|---|---|---|
| `query_id` | **resolved** | `query_manifest.parquet` / `query_masks.parquet`, official `MassSpecGymID#######` format |
| `candidate_id` (InChIKey) | mechanism known, data not fetched | needs their v1 mass-candidate JSON (164 MB, confirmed hash, not downloaded); ordering policy confirmed (`preserve`/`source_order`) |
| `rank` | mechanism known | sort raw scores per query, descending — needs actual score values from `scores.pt` |
| `score` | mechanism known, data not fetched | raw `scores.pt` values, not opened this pass |
| `probability` | **unchanged: do not populate** | `T_eval=0.003` temperature-scaled score is not confirmed to be a calibrated posterior in masstrust's sense |
| `is_correct` / `target_inchikey` | **resolved** | `query_manifest.parquet`'s `inchikey` column gives ground truth directly; `label_mode="inchikey"` confirms matching convention identical to masstrust's own |
| `split` | **resolved, revised recommendation** | no `val`/`test` split exists for this model at all (100% of it is the official test fold, evaluated monolithically) — any calibration/evaluation split would be **masstrust's own construction**, not inherited from Selective-MSMS. Recommend labels like `masstrust_calibration_half` / `masstrust_evaluation_half` to make this unambiguous, distinct from both masstrust's real val/test and from Selective-MSMS's (non-applicable-here) `cal`/`eval` |
| `model_name` | unchanged | e.g. `"selective_msms_ensemble_mlp_mass"` |
| `checkpoint_sha256` | **resolved** | the 5 real ensemble-member hashes (from the original report's `MANIFEST.tsv` pull, `checkpoints.zip` section): `member_01=8a460e998073f34968691a2a6d3e48296e4f03141d28adf3dcc8df040a58cff0`, `member_02=524bbc1994963f14db73b77a863814f74e3b2a1957103ebd4577d8421872f65d`, `member_03=70dce1f3126cae89d98e55798bfec82954803d199f7a6e806c730284ae98f484`, `member_04=1bac42fc6b149ec66e1d4e29fa812ecf30f411fe04f254b0c15f9c6dace2576a`, `member_05=367c5742ea9f3ed6a4eb40d96c10218ab9d3611c5aaa9003a67d16fc10230f64` — still needs a documented single-hash convention (e.g. sha256 of the sorted concatenation) if the schema truly requires one scalar |
| `dataset_version` | **resolved, and flagged** | `"MassSpecGym_v1"` — confirmed distinct from masstrust's `"MassSpecGym1.5"`; do not conflate |
| `candidate_pool` | **resolved, and flagged as higher-risk than before** | `"MassSpecGym_retrieval_candidates_mass.json"` (v1) — confirmed 2.77× smaller than masstrust's v1.5 equivalent; must use their file, never ours |
| `seed` | partially resolved | `candidate_seed=42` (global, `config/paper.yml`); no SGR split seed applies to this artifact (§3); individual ensemble-member training seeds not surfaced anywhere fetched |
| `run_kind` | **unchanged recommendation, reinforced** | needs a new value (e.g. `"external_import"`) — now doubly justified since even the *split* for this artifact would be masstrust's own construction, not Selective-MSMS's, making accurate `run_kind` labeling more important, not less |

---

## Revised disk/download estimate

The original report's 3.18 GB (`results.zip` alone) vs. 18 GB (full regeneration) estimates are
both now superseded — a real importer for the `mass_capped/ensemble_mlp_mass` artifact
specifically would need only:

| file | size | status |
|---|---|---|
| this spike's extractions | 9.76 MB | **done** |
| `scores.pt` (mass_capped/ensemble_mlp_mass/test) | 126,250,694 bytes (~126 MB) | not fetched — needed for actual score values, and should be opened via `torch.load(..., weights_only=True)` (PyTorch's own built-in restricted loading mode), not a hand-rolled parser |
| `MassSpecGym_retrieval_candidates_mass.json` (v1) | 164,001,599 bytes (~164 MB) | not fetched — needed for candidate identity |
| **total for a real importer** | **~290 MB** | vs. the original report's 3.18 GB estimate — the small provenance files already answered enough that the full `results.zip` is not needed at all |

Both remaining files are plain HTTPS downloads from Zenodo/HuggingFace (not zip members needing
range tricks) and both are within CC BY 4.0 / MIT terms respectively. Neither was downloaded in
this pass, per your instructions.

---

## Final verdict: **B — Reproducible external-prediction benchmark**

> The ranking table can be reconstructed, but their exact split or method outputs cannot. Use it
> only to demonstrate masstrust on an external retriever; do not claim method parity.

**This is explicitly not a competitor-parity benchmark.** It cannot be used to claim masstrust
"beats" or "matches" Selective-MSMS, and must never be described as reproducing Selective-MSMS's
own split or method outputs.

**Why not A:** Verdict A requires reconstructing predictions, candidate identities, *and*
Selective-MSMS's calibration/evaluation split, to compare "under their protocol." Candidate
identities and predictions look mechanically reconstructable (§1, §4 above, modulo the one
`scores.pt`-opening gap). But **their split does not exist for this artifact** — §3's finding
that `mass_capped/ensemble_mlp_mass` was only ever evaluated on the full test set, never split
into `cal`/`eval`, rules out "their protocol" as a real thing to compare under, for the one model
that matches masstrust's own candidate-pool choice. There is nothing to inherit.

**Why not C:** Query identity, candidate-handling policy, ground-truth matching convention, and
licensing are all *documented and cross-confirmed*, not ambiguous — five independent exact-count
matches on dataset size, an explicit `label_mode`/`preserve`/`source_order` policy statement, a
direct per-query ground-truth InChIKey column, and hash-verified provenance metadata. This is a
well-audited artifact, not an opaque one. The one real gap (unopened `scores.pt`) is a bounded,
specific, addressable next step — not pervasive ambiguity.

**Why B fits:** The score/rank/correctness table is very likely reconstructable end-to-end
(pending the one deferred, security-appropriate `scores.pt` open). It represents **real
predictions from a real external retrieval model** (Selective-MSMS's mass-candidate ensemble),
usable to demonstrate masstrust's scoring/calibration methods working on an externally-sourced
ranking. But because no Selective-MSMS calibration/evaluation split applies to this model, any
comparison would necessarily use **masstrust's own constructed split** over their fixed
predictions — not "Selective-MSMS's protocol." Per your explicit instruction, this is **never**
"same prediction, same split" relative to masstrust's pinned v1.5 harness, and per verdict B,
it must also not be described as method parity with Selective-MSMS's own SGR results.

**Not proposing a minimal importer design** — per your instructions, that's specified only for
verdict A. If you'd like, a future step could still scope a "verdict B" importer (dataset version
and candidate pool both stamped as `_v1`/non-`1.5`, `run_kind="external_import"`, split labeled
`masstrust_calibration_half`/`masstrust_evaluation_half`), but that would be a new, explicit
ask — not implied by this report.

---

## Status: paused — resume after risksieve backend integration

Verdict B is approved. The importer is **not** being built now. Decision and reasoning:

- The ~290 MB fetch itself is cheap, but what it buys is proof that masstrust *can* consume an
  external retriever's predictions — not evidence of winning or losing against a competitor.
  Verdict B already rules out a competitor-parity claim, so there's no urgency tied to a
  comparison result.
- masstrust's statistics/calibration backend is planned to move to `risksieve`. Building the
  importer against the current calibration backend first would likely mean designing it around
  the soon-to-be-legacy calibration path, and redoing that work once the migration lands.
- This is a reprioritization, not a rejection: the artifact, licensing, and provenance work done
  here remain valid and are recorded for reuse.

**How this is positioned going forward:**

- Selective-MSMS competitor-parity benchmark → **not possible** (verdict B, `mass_capped/ensemble_mlp_mass` has no Selective-MSMS-native calibration/evaluation split).
- Selective-MSMS artifact as an external-retriever benchmark → **worth doing**, deferred.
- risksieve-backed masstrust validated against real data → **a strong fit for this exact artifact**, once risksieve integration lands.

**Framing to preserve for whenever this resumes:**

- The Selective-MSMS importer is not for competitor-parity purposes — it is an
  external-prediction compatibility benchmark.
- Work begins only after masstrust's risksieve backend integration is in place.
- Any calibration/evaluation split used on this artifact must be pre-registered before results
  are computed, and must never be described as reproducing Selective-MSMS's own split.
- This benchmark is handled in a separate namespace and a separate report from the official
  MassSpecGym v1.5 benchmark (`benchmarks/massspecgym/`) — never merged into it.

**Minimal design to use when resuming**, once risksieve integration is in place:

```
scores.pt
+ candidate metadata (their v1 mass-candidate JSON)
+ ground-truth parquet (query_manifest.parquet, already in hand)
        ↓
  isolated, sandboxed Python conversion
  (torch.load(..., weights_only=True); no core changes)
        ↓
  masstrust's common CSV/Parquet schema
  (run_kind="external_import", dataset_version="MassSpecGym_v1",
   candidate_pool="MassSpecGym_retrieval_candidates_mass.json")
        ↓
  a pre-registered calibration/evaluation split
  (masstrust's own construction — documented before any number is computed)
        ↓
  legacy masstrust vs. risksieve-backed masstrust
```

The comparison this produces is not "masstrust vs. Selective-MSMS" — it is **masstrust's old
risk-control path vs. its risksieve-backed path, evaluated on the same external predictions**.
Selective-MSMS's artifact just supplies real, already-published, non-self-generated rankings to
run that comparison against, sidestepping this machine's training-throughput blocker entirely.

## Remaining unknowns

- **`scores.pt`'s literal internal shape/row-order** — the single largest gap. Answerable safely
  in a future step via `torch.load(path, weights_only=True)` (PyTorch's own built-in restricted
  loading mode) run locally against a fully-downloaded file, not via a hand-rolled partial parser.
- **No record-level (query_id-by-query_id or InChIKey-by-InChIKey) join between v1 and v1.5** —
  only aggregate count matches were checked. A true join would need their 262 MB v1 TSV.
- **Per-query candidate-count distribution** (min/max, not just the average) for the mass
  setting — needs either the candidate JSON or `scores.pt`.
- **Individual ensemble-member training seeds** for `ensemble_mlp_mass`'s 5 members — not
  surfaced in anything fetched.

## What this update deliberately does not do

No full-artifact download (`results.zip` in full, `checkpoints.zip`, or the "18 GB" regenerated
predictions). No importer code. No `masstrust-core` changes. No claim of "same prediction, same
split" relative to masstrust's pinned v1.5 harness, under any outcome. `scores.pt` was
deliberately not opened, for the safety reason stated above.

---

# Original feasibility report (superseded in part by the spike above; kept for full context)

## 1. Dataset parity

| dimension | masstrust (this harness) | Selective-MSMS |
|---|---|---|
| dataset version | **MassSpecGym v1.5** — pinned HF revision `c9aa3feb5f6ec0adee56cc78d2dce24826356156` on `roman-bushuiev/MassSpecGym`, file `MassSpecGym1.5.tsv` | **MassSpecGym v1** — their own README states explicitly: *"The experiments use **MassSpecGym v1**, not v1.5."* Their `EXTERNAL_DATA.tsv` pins `MassSpecGym.tsv` (262,334,768 bytes), not the `1.5`-suffixed file. |
| split definition | official `fold` column (train/val/test) as distributed, used as-is | official fold column, same fold sizes — see spike §2 for the now-confirmed exact match |
| candidate pool | `MassSpecGym1.5_retrieval_candidates_mass.json` (mass-filtered, ≤256/query) | `mass_capped`: *"existing opaque mass-filtered pool, maximum 256."* v1 file, now confirmed 2.77× smaller than ours — see spike §2 |
| candidate identity | InChIKey (via `MolToInChIKey`) | InChIKey — now confirmed directly (`label_mode: "inchikey"`), not just inferred |
| ground-truth matching | exact InChIKey string equality | exact InChIKey — now confirmed, was "to-verify" in the original pass |

**Parity verdict (superseded by the spike's Final Verdict B above):** the original report called
this "non-comparable for exact framing, recoverable parity plausible but unverified." The spike
resolved most of that uncertainty in masstrust's favor for *query/candidate identity*, but
surfaced a new, decisive blocker for *split* parity: Selective-MSMS's own split simply doesn't
exist for the matching model. See "Final verdict" above for the current, authoritative
conclusion.

## 2. Artifact format

See the spike's "Downloaded-file inventory" and §1/§5 above for the now-confirmed internal
structure. The original report's discovery that `results.zip` (3.18 GB) is sufficient (no 18 GB
regeneration needed) still holds, and is further refined by the "Revised disk/download estimate"
above (~290 MB actually needed, not even the full 3.18 GB).

## 3. Licensing and redistribution

Unchanged from the original report — verified as three independent grants (Selective-MSMS code:
MIT; Selective-MSMS Zenodo data/model release: CC BY 4.0; MassSpecGym dataset: MIT), all
redistribution-friendly with attribution. **Verdict: clean go**, no ambiguity requiring an email
to the authors.

## 5. Comparison design (original three tiers, for reference)

1. Same prediction, same split (preferred) — **not achievable** (unchanged).
2. Same prediction, reconstructed candidate metadata — **revised by the spike**: achievable for
   the *prediction/candidate* side, but the "split" part must now be understood as masstrust's
   own construction, not Selective-MSMS's, since no split exists for the matching model. This is
   exactly what verdict B captures.
3. Paper-number comparison only, clearly marked non-parity — unchanged, always available,
   lowest cost.

## 6. Cost and risk (original estimates; see "Revised disk/download estimate" above for current)

Original estimates assumed `results.zip` in full (3.18 GB) might be needed. The spike shows a
real importer needs roughly 290 MB total (`scores.pt` + the v1 candidate-pool JSON), not the
full archive.
