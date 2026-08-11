#!/usr/bin/env python3
"""Download and checksum-verify the nexs-metabolomics DNA adductomics database (CC-BY 4.0).

See ../FEASIBILITY.md ss2.1 for what this dataset is and is not, and ../README.md for why
this preflight is not a validated benchmark result. Pinned to a specific commit SHA (not
`main`) for reproducibility -- confirmed via the GitLab API, not assumed.

Every downloaded file's SHA-256 is checked against EXPECTED_SHA256 below (recorded from the
actual bytes served at PINNED_COMMIT during this preflight's own real-data run -- see
../PREFLIGHT_REPORT.md's provenance table). A mismatch is a hard failure: the upstream artifact
changed, or something went wrong in transit, either way not something to silently proceed past.
"""
import argparse
import hashlib
import json
import os
import subprocess
import sys

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

# Recorded directly from this preflight's own real-data run against PINNED_COMMIT above (see
# PREFLIGHT_REPORT.md's provenance table) -- not copied from an upstream-published checksum
# file, since none is published for these paths. Bump only together with PINNED_COMMIT.
EXPECTED_SHA256 = {
    "database.xlsx": "1631952fd5f5271bc9e5d4462169ee39b8fc6ec8da7afd9338601ab84e6c2c42",
    "experimental.html": "36e7d0dd76825d2f4f84842937920a5459c51f26790eac42bb00732ae68d6d4f",
    "predicted.html": "942f249ed5961432cf80132ba8a66cbdf5f6d29428d00f79a4d3786fa523e13b",
    "README_upstream.md": "2b145f5bcbde102ca7b8d9bc0f98bb93cb9d910d960c9eb6ef778221d1859748",
}

HERE = os.path.dirname(os.path.abspath(__file__))


class ChecksumMismatch(Exception):
    pass


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


def verify_checksum(name, digest, url):
    """Raises ChecksumMismatch if `digest` doesn't match EXPECTED_SHA256[name]. Pure/network-free
    so it's directly unit-testable -- see `selftest()` below."""
    expected = EXPECTED_SHA256[name]
    if digest != expected:
        raise ChecksumMismatch(
            f"{name}: expected sha256 {expected}, got {digest} -- the file at "
            f"{url} no longer matches what this preflight was verified against. "
            "Do not proceed on unverified data; re-check the upstream artifact "
            "before updating EXPECTED_SHA256."
        )


def selftest():
    """No network access. Confirms a checksum mismatch is a hard failure, not a silent
    pass-through -- run before ever trusting a real download."""
    try:
        verify_checksum("database.xlsx", "0" * 64, "https://example.invalid/database.xlsx")
    except ChecksumMismatch:
        pass
    else:
        raise AssertionError("verify_checksum did not raise on a wrong digest")

    verify_checksum(
        "database.xlsx", EXPECTED_SHA256["database.xlsx"], "https://example.invalid/database.xlsx"
    )  # must not raise
    print("selftest passed: checksum mismatch is a hard failure, matching digest is not")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out-dir", default=os.path.join(HERE, "..", "data"))
    ap.add_argument(
        "--selftest", action="store_true", help="run the network-free checksum selftest and exit"
    )
    args = ap.parse_args()
    if args.selftest:
        selftest()
        return
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
    try:
        for name, rel_path in FILES.items():
            url = f"{RAW_BASE}/{rel_path}"
            out_path = os.path.join(args.out_dir, name)
            print(f"downloading {url} -> {out_path}")
            download(url, out_path)
            size = os.path.getsize(out_path)
            digest = sha256_file(out_path)
            verify_checksum(name, digest, url)
            manifest["files"][name] = {"url": url, "bytes": size, "sha256": digest}
            print(f"  {size} bytes, sha256 {digest} (matches expected)")
    except ChecksumMismatch as e:
        print(f"CHECKSUM MISMATCH: {e}", file=sys.stderr)
        sys.exit(1)

    manifest_path = os.path.join(args.out_dir, "manifest.json")
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=2)
    print(f"wrote {manifest_path}")


if __name__ == "__main__":
    main()
