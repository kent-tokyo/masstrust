"""Shared parsing for the nexs-metabolomics DNA adductomics database (CC-BY 4.0).

See ../FEASIBILITY.md ss2.1 for what these files are: `database.xlsx` is the 717-compound
master table; `experimental.html` and `predicted.html` are R/DT htmlwidget pages whose actual
per-fragment data is embedded as a JSON array inside the HTML (not in any <table> markup) --
extracted here with a bracket-matched parse, the same way it was first confirmed by hand during
Phase A reconnaissance.
"""
import json
import re

import openpyxl.reader.excel as _rd

# openpyxl chokes on this workbook's broken embedded-drawing relationship (missing image
# target); the workbook itself (all real cell data) is unaffected. See FEASIBILITY.md ss2.1.
_rd.find_images = lambda archive, path: ([], [])
import openpyxl  # noqa: E402

_NUCLEOBASE_RE = re.compile(r"-(d[ACGTUIX])$", re.IGNORECASE)

XLSX_COLUMNS = [
    "name", "short_name", "alt_name", "formula", "mono_mass", "charged_mass",
    "charged_mass_minus_dr", "source", "adduct", "structure_file", "reference",
]


def load_compound_table(xlsx_path):
    """Returns a dict keyed by InChIKey -> compound record (dict)."""
    wb = openpyxl.load_workbook(xlsx_path, data_only=True)
    ws = wb["database"]
    rows = list(ws.iter_rows(min_row=2, values_only=True))
    out = {}
    for r in rows:
        inchikey = r[16]
        if not inchikey:
            continue
        short_name = r[2]
        nb_match = _NUCLEOBASE_RE.search(short_name or "")
        out[inchikey] = {
            "name": r[1],
            "short_name": short_name,
            "formula": (r[4] or "").strip(),
            "mono_mass": r[5],
            "charged_mass": r[6],
            "source": r[8],
            "adduct": r[9],
            "reference": (r[11] or "").strip() or None,
            "smiles": r[14],
            "inchi": r[15],
            "inchikey": inchikey,
            "iupac_name": r[17],
            "nucleobase": nb_match.group(1).lower() if nb_match else None,
        }
    return out


def _extract_embedded_data(html_path):
    """Bracket-matched extraction of the DT htmlwidget's `"data":[[...]]` JSON array.

    Column layout, confirmed directly against both experimental.html and predicted.html during
    Phase A reconnaissance (07_make_experimental_datatable.R / 09_make_predicted_datatable.R in
    the upstream repo build the same 11-column shape for both):
    0 compound_id, 1 name, 2 short_name, 3 alt_name, 4 inchikey, 5 collision_energy,
    6 fragment_mz, 7 relative_intensity, 8 precursor_mass, 9 precursor_mass_alt,
    10 precursor_mass_minus_dr.
    """
    with open(html_path, encoding="utf-8", errors="ignore") as f:
        s = f.read()
    i = s.find('"data":[[')
    if i == -1:
        raise ValueError(f"no embedded data array found in {html_path}")
    start = i + len('"data":')
    depth = 0
    j = start
    while True:
        if s[j] == "[":
            depth += 1
        elif s[j] == "]":
            depth -= 1
            if depth == 0:
                break
        j += 1
    return json.loads(s[start : j + 1])


def load_fragment_table(html_path):
    """Returns {inchikey: {"name":..., "short_name":..., "fragments": [(ce, mz, intensity), ...]}}."""
    cols = _extract_embedded_data(html_path)
    n = len(cols[0])
    by_key = {}
    for k in range(n):
        inchikey = cols[4][k]
        if inchikey is None:
            continue
        rec = by_key.setdefault(
            inchikey, {"name": cols[1][k], "short_name": cols[2][k], "fragments": []}
        )
        ce, mz, intensity = cols[5][k], cols[6][k], cols[7][k]
        if mz is None or intensity is None:
            continue
        rec["fragments"].append((ce, float(mz), float(intensity)))
    return by_key


def merged_spectrum_peaks(fragments, mz_round=2):
    """Collapse a compound's (possibly multi-collision-energy) fragment list into one
    representative peak list: max observed intensity per rounded m/z bin, across all energies.

    ponytail: merging across collision energies (rather than treating each CE as a separate
    spectrum/query) is a deliberate simplification -- it avoids near-duplicate technical-replicate
    queries from the same compound (see README.md's leakage note) at the cost of losing
    per-energy fragmentation detail. Upgrade path: keep energies separate once enough compounds
    exist to make compound-disjoint splitting meaningful at that finer grain (FEASIBILITY.md ss0).
    """
    best = {}
    for _ce, mz, intensity in fragments:
        key = round(mz, mz_round)
        if key not in best or intensity > best[key]:
            best[key] = intensity
    mzs = sorted(best)
    intensities = [best[m] for m in mzs]
    return mzs, intensities
