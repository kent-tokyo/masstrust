#!/usr/bin/env python3
"""Convert query_scores.parquet + split_manifest.json into masstrust's labeled-candidates CSV
schema (see crates/masstrust-core/src/types.rs::Candidate).

Each query becomes exactly one CSV row: its own top-1 candidate. We do not have per-candidate
identity for the rest of the ~254-candidate pool (the exact v1 mass-filtered candidate JSON
Selective-MSMS used is not publicly retrievable -- see ../README.md), so this benchmark cannot
reproduce masstrust's own gap/margin-style scoring methods on this artifact. That is not a gap
here: the comparison this benchmark runs holds the confidence score fixed and varies only the
calibration/certification METHOD (empirical / binomial / legacy-crc / risksieve SDR), which
needs exactly what this row provides -- one confidence score and one correctness label per
query -- and nothing more.

`score` and `probability` are both set to the artifact's own `confidence` column (present in
query_scores.parquet): the top-1 probability from softmax(ensemble_mean_scores / T_eval),
T_eval=0.003 -- verified against Selective-MSMS's own source, see ../README.md. This is a
deliberate choice to reuse their own score transform rather than construct a new one; it is NOT a
claim that this is a calibrated posterior in masstrust's sense (see
benchmarks/selective_msms/PLAN.md's original field-mapping table on that exact point) -- it is
the thing that gets fed into masstrust's OWN calibration methods, which is what this benchmark
is testing.
"""
import hashlib
import json
import os

import pandas as pd

RUN_LABEL = "mlp_mass"
SPLIT = "test"

# Sorted concatenation of the 5 ensemble-member checkpoint hashes recorded in
# benchmarks/selective_msms/PLAN.md ("Resolved field-mapping table", checkpoint_sha256 row).
# No single hash exists upstream for a 5-member ensemble; this is masstrust's own convention,
# recorded here (not silently invented at import time).
_MEMBER_HASHES = sorted([
    "8a460e998073f34968691a2a6d3e48296e4f03141d28adf3dcc8df040a58cff0",
    "524bbc1994963f14db73b77a863814f74e3b2a1957103ebd4577d8421872f65d",
    "70dce1f3126cae89d98e55798bfec82954803d199f7a6e806c730284ae98f484",
    "1bac42fc6b149ec66e1d4e29fa812ecf30f411fe04f254b0c15f9c6dace2576a",
    "367c5742ea9f3ed6a4eb40d96c10218ab9d3611c5aaa9003a67d16fc10230f64",
])
CHECKPOINT_SHA256 = hashlib.sha256("".join(_MEMBER_HASHES).encode()).hexdigest()

HERE = os.path.dirname(os.path.abspath(__file__))
DATA_DIR = os.path.join(HERE, "..", "data")
QUERY_SCORES_PATH = os.path.join(DATA_DIR, "query_scores.parquet")
MANIFEST_PATH = os.path.join(HERE, "..", "split_manifest.json")

CSV_COLUMNS = [
    "query_id", "candidate_id", "rank", "score", "probability", "is_correct",
    "split", "model_name", "checkpoint_sha256", "dataset_version", "candidate_pool",
    "seed", "run_kind", "target_inchikey",
]


def main():
    with open(MANIFEST_PATH) as f:
        manifest = json.load(f)
    assignment_by_qid = {a["query_id"]: a["assignment"] for a in manifest["assignments"]}

    df = pd.read_parquet(QUERY_SCORES_PATH)
    sub = df[(df.run_label == RUN_LABEL) & (df.split == SPLIT) & (df.K == 1)].copy()
    assert set(sub.query_id) == set(assignment_by_qid), "manifest/query_scores.parquet mismatch"

    rows = []
    for row in sub.itertuples():
        rows.append({
            "query_id": row.query_id,
            "candidate_id": f"{row.query_id}_top1",
            "rank": 1,
            "score": row.confidence,
            "probability": row.confidence,
            "is_correct": bool(row.hit == 1.0),
            "split": assignment_by_qid[row.query_id],
            "model_name": "selective_msms_ensemble_mlp_mass",
            "checkpoint_sha256": CHECKPOINT_SHA256,
            "dataset_version": "MassSpecGym_v1",
            "candidate_pool": "MassSpecGym_retrieval_candidates_mass.json",
            "seed": 42,
            "run_kind": "external_import",
            "target_inchikey": row.molecule_group_id,
        })

    out = pd.DataFrame(rows, columns=CSV_COLUMNS)
    # masstrust's Rust CSV deserializer expects lowercase true/false, not pandas' default True/False.
    out["is_correct"] = out["is_correct"].map({True: "true", False: "false"})
    cal = out[out.split == "masstrust_calibration_half"]
    ev = out[out.split == "masstrust_evaluation_half"]

    cal_path = os.path.join(DATA_DIR, "calibration.csv")
    eval_path = os.path.join(DATA_DIR, "evaluation.csv")
    cal.to_csv(cal_path, index=False)
    ev.to_csv(eval_path, index=False)
    print(f"wrote {cal_path}: {len(cal)} rows")
    print(f"wrote {eval_path}: {len(ev)} rows")


if __name__ == "__main__":
    main()
