#!/usr/bin/env python3
"""Pre-registered calibration/evaluation split for the Selective-MSMS external-prediction
benchmark. Grouped by `molecule_group_id` (confirmed identical to the query's 14-character
2D-InChIKey block in ../data/query_scores.parquet) so the same target molecule's queries never
span both halves.

This manifest is written BEFORE any of the 5 comparison methods are run, and is never edited
after creation -- see ../README.md "Pre-registration" section. If this script is re-run, it
must reproduce byte-identical assignments given the same seed and input file.
"""
import hashlib
import json
import os
import sys

import pandas as pd

SEED = 42
CALIBRATION_FRACTION = 0.5  # by molecule group count, not query count
RUN_LABEL = "mlp_mass"
SPLIT = "test"

HERE = os.path.dirname(os.path.abspath(__file__))
DATA_DIR = os.path.join(HERE, "..", "data")
QUERY_SCORES_PATH = os.path.join(DATA_DIR, "query_scores.parquet")
MANIFEST_PATH = os.path.join(HERE, "..", "split_manifest.json")


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    if os.path.exists(MANIFEST_PATH):
        sys.exit(
            f"{MANIFEST_PATH} already exists. The split manifest is immutable once created "
            "-- delete it explicitly (and understand you are re-registering a new split) "
            "before re-running this script."
        )

    df = pd.read_parquet(QUERY_SCORES_PATH)
    sub = df[(df.run_label == RUN_LABEL) & (df.split == SPLIT) & (df.K == 1)].copy()

    groups = sorted(sub.molecule_group_id.unique().tolist())
    rng = __import__("random").Random(SEED)
    shuffled = groups[:]
    rng.shuffle(shuffled)
    n_cal_groups = round(len(shuffled) * CALIBRATION_FRACTION)
    cal_groups = set(shuffled[:n_cal_groups])
    eval_groups = set(shuffled[n_cal_groups:])
    assert cal_groups.isdisjoint(eval_groups)

    assignments = []
    for row in sub.itertuples():
        half = "masstrust_calibration_half" if row.molecule_group_id in cal_groups else "masstrust_evaluation_half"
        assignments.append({
            "query_id": row.query_id,
            "molecule_group_id": row.molecule_group_id,
            "assignment": half,
        })
    assignments.sort(key=lambda a: a["query_id"])

    manifest = {
        "benchmark": "selective_msms_external",
        "purpose": (
            "External-prediction compatibility benchmark comparing masstrust's legacy "
            "calibration methods against risksieve-backed SCoRE-SDR certification, on the "
            "same fixed confidence score. Not a Selective-MSMS competitor-parity benchmark; "
            "does not reproduce Selective-MSMS's own split (none exists for this model "
            "artifact -- see benchmarks/selective_msms/PLAN.md, 'Split reconstruction')."
        ),
        "source_artifact": {
            "zenodo_record": "19108280",
            "zip_member": "data/results/numerical/query_scores.parquet",
            "run_label": RUN_LABEL,
            "split": SPLIT,
            "sha256": sha256_file(QUERY_SCORES_PATH),
            "size_bytes": os.path.getsize(QUERY_SCORES_PATH),
        },
        "split_construction": {
            "method": "group split by molecule_group_id (== 14-char 2D InChIKey block); "
                      "the same target molecule's queries never appear in both halves",
            "seed": SEED,
            "calibration_fraction_of_groups": CALIBRATION_FRACTION,
            "shuffle_algorithm": "random.Random(seed).shuffle over sorted(unique molecule_group_id)",
            "constructed_by": "masstrust, not inherited from Selective-MSMS",
        },
        "counts": {
            "n_molecule_groups_total": len(groups),
            "n_molecule_groups_calibration": len(cal_groups),
            "n_molecule_groups_evaluation": len(eval_groups),
            "n_queries_total": len(assignments),
            "n_queries_calibration": sum(1 for a in assignments if a["assignment"] == "masstrust_calibration_half"),
            "n_queries_evaluation": sum(1 for a in assignments if a["assignment"] == "masstrust_evaluation_half"),
        },
        "assignments": assignments,
    }

    with open(MANIFEST_PATH, "w") as f:
        json.dump(manifest, f, indent=2)
        f.write("\n")

    print(f"Wrote {MANIFEST_PATH}")
    print(json.dumps(manifest["counts"], indent=2))


if __name__ == "__main__":
    main()
