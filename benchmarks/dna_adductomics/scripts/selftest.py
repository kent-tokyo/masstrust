#!/usr/bin/env python3
"""One runnable check for this pipeline, against a tiny synthetic fixture -- no network access,
no real dataset download, no `benchmarks/dna_adductomics/data/` dependency. Mirrors
`benchmarks/massspecgym/smoke_test.py`'s role for that harness.

Builds a synthetic `database.xlsx` / `experimental.html` / `predicted.html` triple with the exact
column layout `adductomics_data.py` expects (confirmed against the real files during Phase A
reconnaissance -- see FEASIBILITY.md ss2.1), then runs export_candidates.py -> validate_data.py ->
run_benchmark.py -> generate_report.py against it exactly as a real run would, and asserts on the
real output files. This checks pipeline *mechanics* (schema, split disjointness, CLI wiring,
report generation) -- it says nothing about scientific correctness on real data, which is what
PREFLIGHT_REPORT.md (real data, separately) is for.

Usage:
    python3 selftest.py
"""
import json
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
MASSTRUST_BIN = os.path.join(REPO_ROOT, "target", "release", "masstrust")

import openpyxl

# 4 synthetic compounds: 2 carry "experimental" fragments (the queries), all 4 are candidates.
# Masses are deliberately close (within the ±0.5 Da mass-filter window) so each query's candidate
# pool includes more than just itself -- exercising the same mass-filter logic as the real run.
COMPOUNDS = [
    # name, short_name, formula, mono_mass, charged_mass, source, adduct, reference, smiles, inchi, inchikey, iupac
    ("Synthetic Adduct A", "SynA-dG", "C11H14N5O7P", 359.06, 360.07, "Synthetic, ROS", "[M+H]+",
     "DOI:10.0000/synthetic-a", "N1C=NC2=C1N=CN=C2N", "InChI=1S/synthetic-a", "AAAAAAAAAAAAAA-AAAAAAAASA-N",
     "synthetic adduct A"),
    ("Synthetic Adduct B", "SynB-dG", "C11H14N5O7P", 359.06, 360.08, "Synthetic, Alkylation", "[M+H]+",
     "DOI:10.0000/synthetic-b", "N1C=NC2=C1N=CN=C2NC", "InChI=1S/synthetic-b", "BBBBBBBBBBBBBB-BBBBBBBBSB-N",
     "synthetic adduct B"),
    ("Synthetic Decoy C", "SynC-dA", "C11H14N5O7P", 359.06, 360.09, "Synthetic, PAH", "",
     "", "N1C=NC2=C1N=CN=C2NCC", "InChI=1S/synthetic-c", "CCCCCCCCCCCCCC-CCCCCCCCSC-N",
     "synthetic decoy C (suspected/theoretical -- no reference)"),
    ("Synthetic Decoy D", "SynD-dC", "C11H14N5O7P", 359.06, 360.10, "Synthetic, HAA", "",
     "DOI:10.0000/synthetic-d", "N1C=NC2=C1N=CN=C2NCCC", "InChI=1S/synthetic-d", "DDDDDDDDDDDDDD-DDDDDDDDSD-N",
     "synthetic decoy D"),
]
EXPERIMENTAL_QUERIES = {"AAAAAAAAAAAAAA-AAAAAAAASA-N", "BBBBBBBBBBBBBB-BBBBBBBBSB-N"}


def build_database_xlsx(path):
    wb = openpyxl.Workbook()
    ws = wb.active
    ws.title = "database"
    header = [None] * 18
    header[1:18] = [
        "Name", "Short name", "Alternative name", "Formula", "Monoisotopic mass",
        "Charged monoisotopic mass", "Charged monoisotopic mass -dR", "Source", "Adduct 1",
        "Structure", "Reference", None, "Structure - mol", "SMILES", "InChI", "InChIKey",
        "IUPAC Name",
    ]
    ws.append(header)
    for i, c in enumerate(COMPOUNDS, start=1):
        (name, short_name, formula, mono_mass, charged_mass, source, adduct, reference,
         smiles, inchi, inchikey, iupac) = c
        row = [None] * 18
        row[0] = i
        row[1] = name
        row[2] = short_name
        row[4] = formula
        row[5] = mono_mass
        row[6] = charged_mass
        row[8] = source
        row[9] = adduct
        row[11] = reference
        row[13] = f"{short_name}.mol"
        row[14] = smiles
        row[15] = inchi
        row[16] = inchikey
        row[17] = iupac
        ws.append(row)
    wb.save(path)


def _html_with_embedded_data(columns):
    """`columns` is the 11-column list-of-lists DT widget layout adductomics_data.py parses.
    Compact (no-space) separators to match the real files' `"data":[[...` formatting exactly --
    `_extract_embedded_data` looks for the literal substring `"data":[[`."""
    payload = json.dumps({"data": columns}, separators=(",", ":"))
    return "<html><body><script>x = " + payload + "</script></body></html>"


def build_experimental_html(path):
    """2 queries (A, B), each with a handful of fragment peaks at one collision energy."""
    ids, names, shorts, alts, inchikeys, ces, mzs, intens, prec, prec2, prec3 = (
        [], [], [], [], [], [], [], [], [], [], []
    )
    fragments_by_query = {
        "AAAAAAAAAAAAAA-AAAAAAAASA-N": [(40, 150.05, 100.0), (40, 200.10, 60.0), (40, 250.15, 30.0)],
        "BBBBBBBBBBBBBB-BBBBBBBBSB-N": [(40, 150.05, 90.0), (40, 210.12, 70.0), (40, 260.20, 20.0)],
    }
    for idx, c in enumerate(COMPOUNDS, start=1):
        inchikey = c[10]
        if inchikey not in fragments_by_query:
            continue
        for ce, mz, intensity in fragments_by_query[inchikey]:
            ids.append(str(idx)); names.append(c[0]); shorts.append(c[1]); alts.append(None)
            inchikeys.append(inchikey); ces.append(ce); mzs.append(mz); intens.append(intensity)
            prec.append(c[4]); prec2.append(c[4] + 1.0); prec3.append(c[4] - 116.0)
    columns = [ids, names, shorts, alts, inchikeys, ces, mzs, intens, prec, prec2, prec3]
    with open(path, "w") as f:
        f.write(_html_with_embedded_data(columns))


def build_predicted_html(path):
    """All 4 compounds get a predicted spectrum (candidate reference library)."""
    ids, names, shorts, alts, inchikeys, ces, mzs, intens, prec, prec2, prec3 = (
        [], [], [], [], [], [], [], [], [], [], []
    )
    for idx, c in enumerate(COMPOUNDS, start=1):
        inchikey = c[10]
        # Each candidate's own predicted spectrum is close to, but not identical to, query A/B's
        # real fragments above -- enough for matchms to compute a nonzero, non-1.0 cosine score.
        base = [150.05, 200.10 + idx, 250.15 - idx]
        for e, mz in enumerate(base):
            ids.append(str(idx)); names.append(c[0]); shorts.append(c[1]); alts.append(None)
            inchikeys.append(inchikey); ces.append(e); mzs.append(mz); intens.append(100.0 - e * 20)
            prec.append(c[4]); prec2.append(c[4] + 1.0); prec3.append(c[4] - 116.0)
    columns = [ids, names, shorts, alts, inchikeys, ces, mzs, intens, prec, prec2, prec3]
    with open(path, "w") as f:
        f.write(_html_with_embedded_data(columns))


def run_py(script, *args):
    result = subprocess.run(
        [sys.executable, os.path.join(HERE, script)] + list(args),
        capture_output=True, text=True,
    )
    return result


def main():
    print("0. build masstrust-cli release binary if missing...")
    if not os.path.exists(MASSTRUST_BIN):
        subprocess.run(["cargo", "build", "--release", "-q", "-p", "masstrust-cli"],
                        cwd=REPO_ROOT, check=True)
    assert os.path.exists(MASSTRUST_BIN), f"{MASSTRUST_BIN} still missing after build"

    with tempfile.TemporaryDirectory() as tmp:
        data_dir = os.path.join(tmp, "data")
        report_dir = os.path.join(tmp, "report")
        os.makedirs(data_dir)

        print("1. build synthetic database.xlsx / experimental.html / predicted.html...")
        build_database_xlsx(os.path.join(data_dir, "database.xlsx"))
        build_experimental_html(os.path.join(data_dir, "experimental.html"))
        build_predicted_html(os.path.join(data_dir, "predicted.html"))

        print("2. export_candidates.py (real matchms scoring, synthetic spectra)...")
        r = run_py("export_candidates.py", "--data-dir", data_dir)
        assert r.returncode == 0, f"export_candidates.py failed:\n{r.stderr}"

        candidates_csv = os.path.join(data_dir, "candidates.csv")
        assert os.path.exists(candidates_csv)
        import csv
        with open(candidates_csv, newline="") as f:
            rows = list(csv.DictReader(f))
        assert rows, "no candidate rows written"
        query_ids = {r["query_id"] for r in rows}
        assert query_ids == EXPERIMENTAL_QUERIES, f"expected {EXPERIMENTAL_QUERIES}, got {query_ids}"
        for row in rows:
            assert row["is_correct"] in ("true", "false"), \
                f"is_correct must be lowercase true/false for masstrust-core's CSV reader, got {row['is_correct']!r}"
            n_cands = sum(1 for r2 in rows if r2["query_id"] == row["query_id"])
            assert n_cands >= 2, "candidate pool of 1 (self-only) defeats score-gap-family scoring"

        print("3. validate_data.py (masstrust validate-split, compound-disjoint check)...")
        r = run_py("validate_data.py", "--data-dir", data_dir)
        assert r.returncode == 0, f"validate_data.py failed:\n{r.stdout}\n{r.stderr}"
        calib_ids, test_ids = set(), set()
        with open(os.path.join(data_dir, "calibration.csv"), newline="") as f:
            calib_ids = {r["query_id"] for r in csv.DictReader(f)}
        with open(os.path.join(data_dir, "test.csv"), newline="") as f:
            test_ids = {r["query_id"] for r in csv.DictReader(f)}
        assert calib_ids and test_ids, "both split halves must be non-empty"
        assert calib_ids.isdisjoint(test_ids), "calibration/test query_id overlap -- leakage"

        print("4. run_benchmark.py (masstrust compare/calibrate/evaluate)...")
        r = run_py("run_benchmark.py", "--data-dir", data_dir, "--out-dir", report_dir)
        assert r.returncode == 0, f"run_benchmark.py failed:\n{r.stdout}\n{r.stderr}"
        assert os.path.exists(os.path.join(report_dir, "run_summary.json"))
        assert os.path.exists(os.path.join(report_dir, "manifest.json"))

        print("5. generate_report.py...")
        r = run_py("generate_report.py", "--data-dir", data_dir, "--report-dir", report_dir)
        assert r.returncode == 0, f"generate_report.py failed:\n{r.stdout}\n{r.stderr}"
        report_path = os.path.join(report_dir, "REPORT.md")
        assert os.path.exists(report_path)
        with open(report_path) as f:
            report_text = f.read()
        assert "PREFLIGHT" in report_text, "report must carry the preflight banner"

    print("\nAll checks passed. This exercises pipeline mechanics against a synthetic fixture --")
    print("it is not evidence about real-data scientific correctness; see PREFLIGHT_REPORT.md")
    print("for the (small-n, non-benchmark) real-data run.")


if __name__ == "__main__":
    main()
