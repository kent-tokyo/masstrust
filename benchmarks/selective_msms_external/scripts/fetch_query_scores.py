#!/usr/bin/env python3
"""Fetch data/results/numerical/query_scores.parquet from Selective-MSMS's Zenodo release
(record 19108280, results.zip) via HTTP range requests, without downloading the full 3.18 GB
archive. Verifies the extracted file's SHA-256 against the archive's own MANIFEST.tsv.

This is the only external file this benchmark downloads. See ../README.md for why scores.pt
and the v1 candidate-pool JSON turned out not to be needed.
"""
import hashlib
import os
import struct
import subprocess
import sys

ZENODO_RECORD = "19108280"
RESULTS_ZIP_URL = f"https://zenodo.org/api/records/{ZENODO_RECORD}/files/results.zip/content"
MANIFEST_URL = f"https://zenodo.org/api/records/{ZENODO_RECORD}/files/MANIFEST.tsv/content"
MEMBER_PATH = "data/results/numerical/query_scores.parquet"
EXPECTED_SHA256 = "f8535c615d062cbccdd484c2416b891559a28b1f1d6d4486f0884ef82b06a6a7"
EXPECTED_SIZE = 32341304

HERE = os.path.dirname(os.path.abspath(__file__))
DATA_DIR = os.path.join(HERE, "..", "data")


def curl_range(start, end, out_path=None):
    args = ["curl", "-sS", "-H", f"Range: bytes={start}-{end}", RESULTS_ZIP_URL]
    if out_path:
        subprocess.run(args + ["-o", out_path], check=True)
        return None
    return subprocess.run(args, check=True, capture_output=True).stdout


def content_length():
    r = subprocess.run(["curl", "-sSI", RESULTS_ZIP_URL], check=True, capture_output=True, text=True)
    for line in r.stdout.splitlines():
        if line.lower().startswith("content-length:"):
            return int(line.split(":", 1)[1].strip())
    raise RuntimeError("no content-length header from Zenodo")


def find_central_directory(total_size):
    tail = curl_range(total_size - 70000, total_size - 1)
    idx = tail.rfind(b"PK\x05\x06")
    if idx == -1:
        raise RuntimeError("EOCD record not found in zip tail")
    cd_size, cd_offset = struct.unpack("<LL", tail[idx + 12:idx + 20])
    return curl_range(cd_offset, cd_offset + cd_size - 1)


def parse_zip64_extra(extra, need_uncomp, need_comp, need_offset):
    i = 0
    while i + 4 <= len(extra):
        tag, size = struct.unpack("<HH", extra[i:i + 4])
        body = extra[i + 4:i + 4 + size]
        if tag == 0x0001:
            j = 0
            uncomp = comp = offset = None
            if need_uncomp:
                uncomp = struct.unpack("<Q", body[j:j + 8])[0]; j += 8
            if need_comp:
                comp = struct.unpack("<Q", body[j:j + 8])[0]; j += 8
            if need_offset:
                offset = struct.unpack("<Q", body[j:j + 8])[0]; j += 8
            return uncomp, comp, offset
        i += 4 + size
    raise AssertionError("ZIP64 extra field required but not found")


def find_member(cd_bytes, member_path):
    SENTINEL = 0xFFFFFFFF
    i = 0
    while i < len(cd_bytes):
        if cd_bytes[i:i + 4] != b"PK\x01\x02":
            break
        (_, _, _, method, _, _, _, comp_size, uncomp_size, name_len, extra_len, comment_len,
         _, _, _, local_header_offset) = struct.unpack("<HHHHHHIIIHHHHHII", cd_bytes[i + 4:i + 46])
        name = cd_bytes[i + 46:i + 46 + name_len].decode("utf-8")
        extra = cd_bytes[i + 46 + name_len:i + 46 + name_len + extra_len]
        if SENTINEL in (comp_size, uncomp_size, local_header_offset):
            z_uncomp, z_comp, z_offset = parse_zip64_extra(
                extra, uncomp_size == SENTINEL, comp_size == SENTINEL, local_header_offset == SENTINEL,
            )
            comp_size = z_comp if z_comp is not None else comp_size
            local_header_offset = z_offset if z_offset is not None else local_header_offset
        if name == member_path:
            assert method == 0, f"{name} is compressed (method={method}); expected STORED"
            return comp_size, local_header_offset
        i += 46 + name_len + extra_len + comment_len
    raise KeyError(f"{member_path} not found in central directory")


def fetch_member(comp_size, local_header_offset, out_path):
    header = curl_range(local_header_offset, local_header_offset + 29 + 4096)
    name_len, extra_len = struct.unpack("<HH", header[26:30])
    data_start = local_header_offset + 30 + name_len + extra_len
    curl_range(data_start, data_start + comp_size - 1, out_path=out_path)


def main():
    os.makedirs(DATA_DIR, exist_ok=True)
    out_path = os.path.join(DATA_DIR, "query_scores.parquet")

    # Cross-check the expected hash against the archive's own published manifest first.
    manifest = subprocess.run(["curl", "-sS", MANIFEST_URL], check=True, capture_output=True, text=True).stdout
    manifest_line = next((l for l in manifest.splitlines() if MEMBER_PATH in l), None)
    if manifest_line is None:
        sys.exit(f"MANIFEST.tsv no longer lists {MEMBER_PATH} -- archive layout changed, stop.")
    manifest_hash = manifest_line.strip().split("\t")[-1]
    if manifest_hash != EXPECTED_SHA256:
        sys.exit(f"MANIFEST.tsv hash {manifest_hash} != pinned {EXPECTED_SHA256} -- archive changed, stop.")

    total = content_length()
    cd_bytes = find_central_directory(total)
    comp_size, local_header_offset = find_member(cd_bytes, MEMBER_PATH)
    assert comp_size == EXPECTED_SIZE, f"size mismatch: {comp_size} != {EXPECTED_SIZE}"

    fetch_member(comp_size, local_header_offset, out_path)

    actual_size = os.path.getsize(out_path)
    actual_hash = hashlib.sha256(open(out_path, "rb").read()).hexdigest()
    assert actual_size == EXPECTED_SIZE, f"downloaded size {actual_size} != {EXPECTED_SIZE}"
    assert actual_hash == EXPECTED_SHA256, f"downloaded sha256 {actual_hash} != {EXPECTED_SHA256}"
    print(f"OK: {out_path} ({actual_size} bytes, sha256 {actual_hash})")


if __name__ == "__main__":
    main()
