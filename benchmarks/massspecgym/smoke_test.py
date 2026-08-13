#!/usr/bin/env python3
"""One runnable check for this pipeline: validate -> evaluate -> report,
against a tiny synthetic fixture (fixtures/{val,test}_predictions.csv).

Does NOT touch massspecgym/torch/rdkit or the real dataset — it only
exercises validate_predictions.py, generate_report.py, and the masstrust
CLI itself. Run this before ever pointing the pipeline at the real
~231k-spectrum dataset.

Usage:
    python smoke_test.py
"""
import csv
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIXTURES = HERE / "fixtures"


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(HERE / args[0])] + list(args[1:]),
        capture_output=True,
        text=True,
    )


def write_variant(src: Path, dst: Path, mutate) -> None:
    with open(src, newline="") as f:
        rows = list(csv.DictReader(f))
    rows = mutate(rows)
    with open(dst, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=rows[0].keys())
        writer.writeheader()
        writer.writerows(rows)


class _CountingTransform:
    # Module level (not defined inside main()) so it can be pickled -- the
    # same DataLoader-worker constraint _CachingTransform itself exists to
    # satisfy (see run_baseline.py's _RetrievalDatasetWithCandidates
    # docstring for the original discovery of this exact pickling pitfall).
    def __init__(self):
        self.calls = 0

    def from_smiles(self, mol):
        self.calls += 1
        return f"result-for-{mol}"


def main() -> None:
    good_val = FIXTURES / "val_predictions.csv"
    good_test = FIXTURES / "test_predictions.csv"

    print("1. validate_predictions.py accepts the clean fixture...")
    result = run("validate_predictions.py", "--val", str(good_val), "--test", str(good_test))
    assert result.returncode == 0, f"expected success, got:\n{result.stderr}"
    # Fixtures bake in one deliberately-duplicated target_inchikey (v3/t3) — answer
    # leakage is warned about, not hard-failed (see validate_split.rs), so this must
    # still exit 0 while surfacing the warning.
    assert "ANSWER LEAKAGE" in result.stderr, f"expected leakage warning, got:\n{result.stderr}"
    print("   OK")

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)

        print("2. validate_predictions.py rejects a duplicate rank...")
        bad = tmp / "dup_rank.csv"
        write_variant(good_test, bad, lambda rows: rows + [dict(rows[0])])
        result = run("validate_predictions.py", "--val", str(good_val), "--test", str(bad))
        assert result.returncode != 0, "expected failure on duplicate rank"
        assert "duplicate rank" in result.stderr
        print("   OK")

        print("3. validate_predictions.py rejects a non-finite score...")
        bad = tmp / "nonfinite_score.csv"

        def inject_nan(rows):
            rows = [dict(r) for r in rows]
            rows[0]["score"] = "nan"
            return rows

        write_variant(good_test, bad, inject_nan)
        result = run("validate_predictions.py", "--val", str(good_val), "--test", str(bad))
        assert result.returncode != 0, "expected failure on non-finite score"
        assert "non-finite" in result.stderr
        print("   OK")

        print("4. validate_predictions.py rejects a query with no true candidate...")
        bad = tmp / "no_true_candidate.csv"

        def strip_true(rows):
            rows = [dict(r) for r in rows]
            for r in rows:
                if r["query_id"] == "t1":
                    r["is_correct"] = "false"
            return rows

        write_variant(good_test, bad, strip_true)
        result = run("validate_predictions.py", "--val", str(good_val), "--test", str(bad))
        assert result.returncode != 0, "expected failure on missing true candidate"
        assert "no true candidate" in result.stderr
        print("   OK")

    print("5. generate_report.py runs end to end on the clean fixture...")
    with tempfile.TemporaryDirectory() as tmp:
        out_dir = Path(tmp) / "report"
        result = run(
            "generate_report.py",
            "--val", str(good_val),
            "--test", str(good_test),
            "--out-dir", str(out_dir),
            "--bootstrap", "200",
        )
        assert result.returncode == 0, f"expected success, got:\n{result.stderr}"
        report_csv = out_dir / "report.csv"
        report_md = out_dir / "report.md"
        assert report_csv.exists(), "report.csv was not written"
        assert report_md.exists(), "report.md was not written"

        with open(report_csv, newline="") as f:
            rows = list(csv.DictReader(f))
        assert len(rows) == 4 * 3, f"expected 4 methods x 3 target risks, got {len(rows)}"
        for row in rows:
            coverage = float(row["test_coverage_achieved"])
            assert 0.0 <= coverage <= 1.0, f"coverage out of range: {row}"
            if row["method"] == "candidate-count":
                assert float(row["unscoreable_rate"]) == 0.0, "candidate-count must always be scoreable"
    print("   OK")

    print("6. _CachingTransform memoizes by SMILES and respects maxsize...")
    import pickle as _pickle

    from run_baseline import _CachingTransform

    inner = _CountingTransform()
    cached = _CachingTransform(inner, maxsize=2)
    assert cached("a") == "result-for-a"
    assert cached("a") == "result-for-a"
    assert inner.calls == 1, "repeated input must hit the cache, not recompute"
    cached("b")
    assert inner.calls == 2
    cached("c")  # cache is now at maxsize=2 (a, b); c is computed but not stored
    assert inner.calls == 3
    assert len(cached._cache) == 2, "cache must not grow past maxsize"
    cached("c")  # not cached -> recomputed
    assert inner.calls == 4, "evicted-by-maxsize entries must recompute, not error"

    info = cached.cache_info()
    # calls above: a(miss,admit) a(hit) b(miss,admit) c(miss,full->reject) c(miss,full->reject)
    assert info == {
        "hits": 1, "misses": 4, "admitted": 2, "rejected_after_full": 2,
        "current_size": 2, "maxsize": 2,
    }, f"cache_info() counters wrong: {info}"
    print("   OK (maxsize bound + recompute-after-full + hit/miss/admitted/rejected counters)")

    print("6b. _CachingTransform(maxsize=None) never evicts...")
    inner_unbounded = _CountingTransform()
    unbounded = _CachingTransform(inner_unbounded, maxsize=None)
    for i in range(50):
        unbounded(f"x{i}")
    assert inner_unbounded.calls == 50
    assert len(unbounded._cache) == 50, "maxsize=None must never evict"
    unbounded("x0")
    assert inner_unbounded.calls == 50, "maxsize=None must still cache everything seen"
    print("   OK")

    print("6c. _CachingTransform rejects a negative maxsize...")
    try:
        _CachingTransform(_CountingTransform(), maxsize=-1)
        raise AssertionError("expected ValueError for negative maxsize")
    except ValueError:
        pass
    print("   OK")

    print("6d. _CachingTransform(maxsize=0) computes every call, admits nothing "
          "(disabled-cache parity with the unwrapped transform)...")
    inner_direct = _CountingTransform()
    inner_wrapped = _CountingTransform()
    disabled = _CachingTransform(inner_wrapped, maxsize=0)
    for mol in ["p", "p", "q", "p"]:
        assert disabled(mol) == inner_direct.from_smiles(mol)
    assert inner_wrapped.calls == inner_direct.calls == 4, "maxsize=0 must recompute every call"
    assert disabled.cache_info()["current_size"] == 0
    print("   OK")

    print("6e. _CachingTransform survives a serialization round trip (DataLoader "
          "worker requirement: num_workers>0 must serialize the dataset, "
          "including its transforms, to hand off to worker processes)...")
    # A fresh (empty) cache must round-trip -- this is the actual constraint in
    # practice, since DataLoader workers are spawned once, before any items
    # have been fetched.
    empty = _CachingTransform(_CountingTransform(), maxsize=10)
    restored = _pickle.loads(_pickle.dumps(empty))
    assert restored("z") == "result-for-z"
    assert restored.cache_info()["current_size"] == 1
    print("   OK")

    try:
        import numpy as np

        print("6f. _CachingTransform(pack_bits=True) round-trips a binary "
              "fingerprint-shaped array exactly, and preserves dtype...")

        class _FakeFingerprinter:
            def from_smiles(self, mol):
                rng = np.random.RandomState(abs(hash(mol)) % (2**31))
                return rng.randint(0, 2, size=64).astype(np.int32)

        uncached = _FakeFingerprinter()
        packed_cache = _CachingTransform(
            _FakeFingerprinter(), maxsize=10, pack_bits=True, unpack_length=64
        )
        for mol in ["m1", "m2", "m1"]:  # m1 twice: exercises both miss and hit paths
            direct = uncached.from_smiles(mol)
            via_cache = packed_cache.from_smiles(mol)
            assert np.array_equal(direct, via_cache), f"packed round-trip mismatch for {mol}"
            assert via_cache.dtype == np.int32, f"dtype changed after unpack: {via_cache.dtype}"
        print("   OK")
    except ImportError:
        print("6f. skipped (numpy not installed) -- pack_bits is only exercised in "
              "the real massspecgym venv, matching this script's own no-numpy-required design.")

    print("\nAll smoke tests passed.")


if __name__ == "__main__":
    main()
