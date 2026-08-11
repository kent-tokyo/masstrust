#!/usr/bin/env python3
"""Download and checksum the nexs-metabolomics DNA adductomics database (CC-BY 4.0).

See ../FEASIBILITY.md ss2.1 for what this dataset is and is not, and ../README.md for why
this benchmark runs as a preflight, not a validated result. Pinned to a specific commit SHA
(not `main`) for reproducibility -- confirmed via the GitLab API, not assumed.
"""
import argparse
import hashlib
import json
import os
import subprocess

REPO = "nexs-metabolomics/projects/dna_adductomics_database"
# Pinned via `GET /projects/<repo>/repository/commits?ref_name=main&per_page=1` on 2026-08-11.
# Bump deliberately (re-run that query), never silently.
PINNED_COMMIT = "15db61a372676fd6fa5e64b2076681a41f187cf4"
RAW_BASE = f"https://gitlab.com/{REPO}/-/raw/{PINNED_COMMIT}"

FILES = {
    "database.xlsx": "public/Database%20for%20MS.xlsx",
    "experimental.html": "public/experimental.html",
    "predicted.html": "public/predicted.html",
    "README_upstream.md": "README.md",
}

HERE = os.path.dirname(os.path.abspath(__file__))


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def download(url, out_path):
    # curl (system cert store), not urllib -- matches the existing convention in
    # benchmarks/selective_msms_external/scripts/fetch_query_scores.py, and avoids
    # this machine's python.org-framework-build SSL cert store gap with urllib.
    subprocess.run(["curl", "-sSL", "--fail", url, "-o", out_path], check=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out-dir", default=os.path.join(HERE, "..", "data"))
    args = ap.parse_args()
    os.makedirs(args.out_dir, exist_ok=True)

    manifest = {
        "source": "nexs-metabolomics DNA adductomics database",
        "source_repo": f"https://gitlab.com/{REPO}",
        "pinned_commit": PINNED_COMMIT,
        "license": "CC-BY 4.0",
        "citation": "La Barbera G et al., \"A Comprehensive Database for DNA Adductomics\", "
                     "Frontiers in Chemistry 2022",
        "files": {},
    }
    for name, rel_path in FILES.items():
        url = f"{RAW_BASE}/{rel_path}"
        out_path = os.path.join(args.out_dir, name)
        print(f"downloading {url} -> {out_path}")
        download(url, out_path)
        size = os.path.getsize(out_path)
        digest = sha256_file(out_path)
        manifest["files"][name] = {"url": url, "bytes": size, "sha256": digest}
        print(f"  {size} bytes, sha256 {digest}")

    manifest_path = os.path.join(args.out_dir, "manifest.json")
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=2)
    print(f"wrote {manifest_path}")


if __name__ == "__main__":
    main()
