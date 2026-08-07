#!/usr/bin/env python3
"""Fail-fast schema validation for the `mlp_mass`/`test`/`K==1` subset of
query_scores.parquet, run before the split manifest or CSVs are built.

Every check here is a hard error (raises), never a warning-and-continue -- an
unexpected schema on this external artifact should stop the benchmark, not
silently produce a plausible-looking but wrong result.

Run standalone for a self-test against synthetic data (no external file needed):
    python3 validate_source_schema.py --selftest
"""
import argparse
import hashlib
import sys

import pandas as pd

EXPECTED_N_QUERIES = 17_556
EXPECTED_N_MOLECULE_GROUPS = 2_998
EXPECTED_CANDIDATE_COUNT_SUM = 4_457_058
EXPECTED_SOURCE_SHA256 = "f8535c615d062cbccdd484c2416b891559a28b1f1d6d4486f0884ef82b06a6a7"


class SchemaValidationError(Exception):
    pass


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def validate_source_file(parquet_path, expected_sha256=EXPECTED_SOURCE_SHA256):
    actual = sha256_file(parquet_path)
    if actual != expected_sha256:
        raise SchemaValidationError(
            f"source file sha256 mismatch: expected {expected_sha256}, got {actual} "
            f"-- the Zenodo artifact changed or the wrong file was fetched, stop."
        )


def validate_mlp_mass_test_k1(df, run_label="mlp_mass", split="test"):
    """`df` is the FULL query_scores.parquet (all run_labels/splits/K). Returns the
    validated K==1 subset for `run_label`/`split`, or raises SchemaValidationError."""
    sub = df[(df.run_label == run_label) & (df.split == split) & (df.K == 1)]

    if len(sub) != EXPECTED_N_QUERIES:
        raise SchemaValidationError(
            f"expected {EXPECTED_N_QUERIES} rows for run_label={run_label!r} split={split!r} "
            f"K=1, got {len(sub)}"
        )
    if sub.query_id.isna().any():
        raise SchemaValidationError("query_id has null values")
    if sub.query_id.nunique() != len(sub):
        raise SchemaValidationError(
            f"query_id is not unique: {sub.query_id.nunique()} unique of {len(sub)} rows"
        )
    if sub.molecule_group_id.isna().any():
        raise SchemaValidationError("molecule_group_id has null values")
    if sub.confidence.isna().any():
        raise SchemaValidationError("confidence has null values")
    if not sub.confidence.apply(lambda x: pd.notna(x) and float("-inf") < x < float("inf")).all():
        raise SchemaValidationError("confidence has non-finite values")
    bad_hit = ~sub.hit.isin([0.0, 1.0])
    if bad_hit.any():
        raise SchemaValidationError(
            f"hit has values other than exactly 0 or 1: {sub.hit[bad_hit].unique().tolist()}"
        )
    n_groups = sub.molecule_group_id.nunique()
    if n_groups != EXPECTED_N_MOLECULE_GROUPS:
        raise SchemaValidationError(
            f"expected {EXPECTED_N_MOLECULE_GROUPS} unique molecule_group_id, got {n_groups}"
        )
    candidate_sum = int(sub.candidate_count.sum())
    if candidate_sum != EXPECTED_CANDIDATE_COUNT_SUM:
        raise SchemaValidationError(
            f"expected candidate_count sum {EXPECTED_CANDIDATE_COUNT_SUM}, got {candidate_sum}"
        )

    return sub


def _selftest():
    import os
    import tempfile

    good = pd.DataFrame({
        "query_id": [f"q{i}" for i in range(3)],
        "molecule_group_id": ["g0", "g0", "g1"],
        "confidence": [0.1, 0.2, 0.9],
        "hit": [0.0, 1.0, 1.0],
        "candidate_count": [10, 10, 5],
        "run_label": ["mlp_mass"] * 3,
        "split": ["test"] * 3,
        "K": [1, 1, 1],
    })

    global EXPECTED_N_QUERIES, EXPECTED_N_MOLECULE_GROUPS, EXPECTED_CANDIDATE_COUNT_SUM
    saved = (EXPECTED_N_QUERIES, EXPECTED_N_MOLECULE_GROUPS, EXPECTED_CANDIDATE_COUNT_SUM)
    EXPECTED_N_QUERIES, EXPECTED_N_MOLECULE_GROUPS, EXPECTED_CANDIDATE_COUNT_SUM = 3, 2, 25

    try:
        out = validate_mlp_mass_test_k1(good)
        assert len(out) == 3, "valid input should pass"

        bad_null_qid = good.copy()
        bad_null_qid.loc[0, "query_id"] = None
        try:
            validate_mlp_mass_test_k1(bad_null_qid)
            raise AssertionError("null query_id should have raised")
        except SchemaValidationError:
            pass

        bad_dup_qid = good.copy()
        bad_dup_qid.loc[1, "query_id"] = "q0"
        try:
            validate_mlp_mass_test_k1(bad_dup_qid)
            raise AssertionError("duplicate query_id should have raised")
        except SchemaValidationError:
            pass

        bad_hit = good.copy()
        bad_hit.loc[0, "hit"] = 0.5
        try:
            validate_mlp_mass_test_k1(bad_hit)
            raise AssertionError("hit=0.5 should have raised")
        except SchemaValidationError:
            pass

        bad_nan_conf = good.copy()
        bad_nan_conf.loc[0, "confidence"] = float("nan")
        try:
            validate_mlp_mass_test_k1(bad_nan_conf)
            raise AssertionError("NaN confidence should have raised")
        except SchemaValidationError:
            pass

        bad_count = good.copy()
        bad_count.loc[0, "candidate_count"] = 999
        try:
            validate_mlp_mass_test_k1(bad_count)
            raise AssertionError("wrong candidate_count sum should have raised")
        except SchemaValidationError:
            pass

        with tempfile.NamedTemporaryFile(delete=False) as f:
            f.write(b"hello world")
            tmp_path = f.name
        try:
            validate_source_file(tmp_path, expected_sha256="0" * 64)
            raise AssertionError("wrong file hash should have raised")
        except SchemaValidationError:
            pass
        finally:
            os.unlink(tmp_path)

        print("OK: all self-test cases behaved as expected")
    finally:
        EXPECTED_N_QUERIES, EXPECTED_N_MOLECULE_GROUPS, EXPECTED_CANDIDATE_COUNT_SUM = saved


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--selftest", action="store_true")
    args = parser.parse_args()
    if args.selftest:
        _selftest()
    else:
        sys.exit("run with --selftest, or import validate_mlp_mass_test_k1/validate_source_file")
