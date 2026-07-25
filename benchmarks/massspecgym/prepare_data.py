#!/usr/bin/env python3
"""Download and pin the official MassSpecGym retrieval-task artifacts.

Fetches the official dataset TSV and the official "standard" (mass-filtered,
<=256 candidates/query) retrieval candidate pool at a pinned HuggingFace
revision (see config.py), and records their identity in manifest.json.

This step does *not* join candidates or compute correctness labels: that is
left to massspecgym's own RetrievalDataset (see run_baseline.py), so we never
reimplement — and risk diverging from — its official correctness convention.

Usage:
    python prepare_data.py --out-dir ./data
"""
import argparse
import hashlib
import json
import platform
import sys
from datetime import datetime, timezone
from pathlib import Path

import config


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", type=Path, default=Path("data"))
    args = parser.parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)

    try:
        from huggingface_hub import hf_hub_download
        import massspecgym
    except ImportError as e:
        sys.exit(
            f"Missing dependency ({e}). Install with:\n"
            f"  pip install -r requirements.txt"
        )

    print(f"Downloading {config.DATASET_FILE} @ {config.DATASET_REVISION[:12]}...")
    dataset_pth = hf_hub_download(
        repo_id=config.HF_REPO_ID,
        repo_type=config.HF_REPO_TYPE,
        filename=f"data/{config.DATASET_FILE}",
        revision=config.DATASET_REVISION,
        local_dir=args.out_dir,
    )
    print(f"Downloading {config.CANDIDATES_FILE} @ {config.DATASET_REVISION[:12]}...")
    candidates_pth = hf_hub_download(
        repo_id=config.HF_REPO_ID,
        repo_type=config.HF_REPO_TYPE,
        filename=f"data/{config.CANDIDATES_FILE}",
        revision=config.DATASET_REVISION,
        local_dir=args.out_dir,
    )

    # Not massspecgym.utils.load_massspecgym(): it takes no path argument and
    # always downloads its own unpinned copy of the *older* "MassSpecGym.tsv"
    # internally (confirmed against the installed package — see config.py).
    # Read our own pinned file directly; only the fold column is needed here.
    import pandas as pd

    df = pd.read_csv(dataset_pth, sep="\t")
    if config.SPLIT_COLUMN not in df.columns:
        sys.exit(f"Expected split column '{config.SPLIT_COLUMN}' not found in dataset.")
    fold_counts = df[config.SPLIT_COLUMN].value_counts().to_dict()
    unexpected_folds = set(fold_counts) - set(config.SPLITS)
    if unexpected_folds:
        print(
            f"WARNING: unexpected fold values {unexpected_folds} beyond "
            f"{config.SPLITS} — dataset schema may have changed upstream.",
            file=sys.stderr,
        )

    manifest = {
        "dataset_version": config.DATASET_VERSION,
        "dataset_revision": config.DATASET_REVISION,
        "dataset_file": config.DATASET_FILE,
        "dataset_sha256": sha256_of(Path(dataset_pth)),
        "candidate_pool": config.CANDIDATE_POOL,
        "candidates_file": config.CANDIDATES_FILE,
        "candidates_sha256": sha256_of(Path(candidates_pth)),
        "fold_counts": fold_counts,
        "massspecgym_package_version": massspecgym.__version__
        if hasattr(massspecgym, "__version__")
        else config.MASSSPECGYM_PACKAGE_VERSION,
        "python_version": platform.python_version(),
        "platform": platform.platform(),
        "prepared_at": datetime.now(timezone.utc).isoformat(),
        "commands": [" ".join(sys.argv)],
    }
    manifest_pth = args.out_dir / "manifest.json"
    manifest_pth.write_text(json.dumps(manifest, indent=2))
    print(f"Wrote manifest to {manifest_pth}")
    print(f"Fold counts: {fold_counts}")


if __name__ == "__main__":
    main()
