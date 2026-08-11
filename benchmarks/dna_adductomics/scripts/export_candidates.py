#!/usr/bin/env python3
"""Build masstrust's labeled-candidates CSV from the real experimental DNA-adduct spectra and
the real CFM-ID-predicted candidate library (see ../FEASIBILITY.md ss2, ../README.md protocol
steps 2-4).

**This is a preflight, not a benchmark** -- see ../README.md and ../FEASIBILITY.md ss2.4 for why:
8 distinct query compounds fails this project's own pre-registered minimum-n floor
(FEASIBILITY.md ss0). Every row is stamped `run_kind=preflight`.

External scoring: matchms CosineGreedy cosine similarity between each query's real,
instrument-acquired spectrum (merged across collision energies -- see adductomics_data.py's
`merged_spectrum_peaks`) and each mass-filtered candidate's real CFM-ID in-silico predicted
spectrum. This is experimental-vs-predicted similarity, not library-vs-library matching --
labelled as such in the `evidence_kind` column, never conflated with a calibrated probability
(masstrust's `probability` column is deliberately left unset; see ../README.md protocol step 6).
"""
import argparse
import hashlib
import os
import sys

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from adductomics_data import load_compound_table, load_fragment_table, merged_spectrum_peaks

from matchms import Spectrum
from matchms.similarity import CosineGreedy

HERE = os.path.dirname(os.path.abspath(__file__))
MASS_TOLERANCE_DA = 0.5  # ponytail: generous fixed window over the real charged-mass column,
# not a peak-picking tolerance -- see README.md protocol step 2. Chosen empirically (see git
# history / conversation) to keep every query's real candidate pool in the 2-13 range: wide
# enough to include real same-nucleobase / same-formula-class confusable candidates, narrow
# enough not to just dump all 579 compounds in every pool. Upgrade path: switch to a per-adduct
# formula/nucleobase-aware pool once enough experimental queries exist to make that distinction
# testable (FEASIBILITY.md ss0/ss2.3).

CSV_COLUMNS = [
    "query_id", "candidate_id", "rank", "score", "is_correct",
    "ground_truth_tier", "evidence_kind", "reference_doi", "candidate_origin",
    "genotoxicant_class", "nucleobase", "precursor_mz", "charge", "instrument",
    "collision_energy", "dataset_version", "source_accession", "run_kind",
]


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def build_spectrum(mzs, intensities, precursor_mz):
    return Spectrum(
        mz=np.array(mzs, dtype=float),
        intensities=np.array(intensities, dtype=float),
        metadata={"precursor_mz": float(precursor_mz)},
    )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data-dir", default=os.path.join(HERE, "..", "data"))
    ap.add_argument("--out", default=None)
    ap.add_argument("--mass-tolerance", type=float, default=MASS_TOLERANCE_DA)
    args = ap.parse_args()
    out_path = args.out or os.path.join(args.data_dir, "candidates.csv")

    xlsx_path = os.path.join(args.data_dir, "database.xlsx")
    exp_path = os.path.join(args.data_dir, "experimental.html")
    pred_path = os.path.join(args.data_dir, "predicted.html")

    compounds = load_compound_table(xlsx_path)
    experimental = load_fragment_table(exp_path)
    predicted = load_fragment_table(pred_path)

    dataset_version = "nexs-metabolomics-dna-adduct-db@15db61a372676fd6fa5e64b2076681a41f187cf4"
    source_accession = "gitlab.com/nexs-metabolomics/projects/dna_adductomics_database"

    # Pre-build one merged predicted spectrum per candidate compound (real CFM-ID predictions).
    candidate_spectra = {}
    for inchikey, rec in predicted.items():
        if inchikey not in compounds:
            continue
        mzs, intensities = merged_spectrum_peaks(rec["fragments"])
        if not mzs:
            continue
        precursor = compounds[inchikey]["charged_mass"]
        if precursor is None:
            continue
        candidate_spectra[inchikey] = build_spectrum(mzs, intensities, precursor)

    cosine = CosineGreedy(tolerance=0.01)
    rows = []
    pool_sizes = {}

    query_keys = sorted(k for k in experimental if k in compounds)
    for query_ik in query_keys:
        qcompound = compounds[query_ik]
        qmass = qcompound["charged_mass"]
        if qmass is None:
            continue
        q_mzs, q_intensities = merged_spectrum_peaks(experimental[query_ik]["fragments"])
        q_spectrum = build_spectrum(q_mzs, q_intensities, qmass)
        ces_observed = sorted({ce for ce, _mz, _i in experimental[query_ik]["fragments"] if ce is not None})

        pool = [
            cik for cik, crec in compounds.items()
            if cik in candidate_spectra and crec["charged_mass"] is not None
            and abs(crec["charged_mass"] - qmass) <= args.mass_tolerance
        ]
        if query_ik not in pool:
            pool.append(query_ik)
        pool_sizes[query_ik] = len(pool)

        scored = []
        for cik in pool:
            result = cosine.pair(q_spectrum, candidate_spectra[cik])
            scored.append((cik, float(result["score"]), int(result["matches"])))
        scored.sort(key=lambda t: t[1], reverse=True)

        for rank, (cik, score, _matches) in enumerate(scored, start=1):
            crec = compounds[cik]
            if cik == query_ik:
                candidate_origin = "true_match"
            elif crec["reference"]:
                candidate_origin = "literature_confirmed"
            else:
                candidate_origin = "suspected_theoretical"
            rows.append({
                "query_id": query_ik,
                "candidate_id": cik,
                "rank": rank,
                "score": score,
                # masstrust-core's CSV reader only accepts lowercase true/false (confirmed against
                # crates/masstrust-core/src/io.rs); Python's csv module would otherwise write
                # str(bool) as "True"/"False" -- same gotcha already documented in
                # benchmarks/massspecgym/README.md's "Known issues" section.
                "is_correct": "true" if cik == query_ik else "false",
                "ground_truth_tier": "reference_standard",
                "evidence_kind": "experimental_spectrum_vs_cfmid_predicted_spectrum",
                "reference_doi": crec["reference"] or "",
                "candidate_origin": candidate_origin,
                "genotoxicant_class": qcompound["source"] or "",
                "nucleobase": qcompound["nucleobase"] or "",
                "precursor_mz": qmass,
                "charge": 1,
                "instrument": "Waters Vion IM-QTOF (Acquity UHPLC, ESI+)",
                "collision_energy": ";".join(str(c) for c in ces_observed),
                "dataset_version": dataset_version,
                "source_accession": source_accession,
                "run_kind": "preflight",
            })

    import csv
    with open(out_path, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=CSV_COLUMNS)
        w.writeheader()
        w.writerows(rows)
    print(f"wrote {len(rows)} rows for {len(query_keys)} queries to {out_path}")
    print("candidate pool sizes by query:", pool_sizes)

    # Compound-disjoint calibration/test split. n=8 distinct compounds is too small for a
    # meaningful random split to matter (FEASIBILITY.md ss0/ss2.3) -- a fixed, deterministic,
    # alphabetical-by-InChIKey first-half/second-half assignment is used instead of a seeded
    # random shuffle, and documented here as arbitrary rather than dressed up as a principled
    # stratification. Never re-run with a different split and call it a re-benchmark: the split
    # itself, not the seed, is the thing to change deliberately if it ever needs revisiting.
    half = (len(query_keys) + 1) // 2
    calibration_ids = set(query_keys[:half])
    calib_rows = [r for r in rows if r["query_id"] in calibration_ids]
    test_rows = [r for r in rows if r["query_id"] not in calibration_ids]
    for name, split_rows in (("calibration.csv", calib_rows), ("test.csv", test_rows)):
        split_path = os.path.join(args.data_dir, name)
        with open(split_path, "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=CSV_COLUMNS)
            w.writeheader()
            w.writerows(split_rows)
        print(f"wrote {len(split_rows)} rows ({len(set(r['query_id'] for r in split_rows))} "
              f"queries) to {split_path}")


if __name__ == "__main__":
    main()
