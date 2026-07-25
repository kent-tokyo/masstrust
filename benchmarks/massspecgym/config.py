"""Pinned identifiers for the MassSpecGym artifacts this benchmark uses.

Verified by reading github.com/pluskal-lab/MassSpecGym (main, 2026-07) and the
huggingface.co/datasets/roman-bushuiev/MassSpecGym API — not guessed. See README.md
for how each value was confirmed.
"""

# HuggingFace dataset repo backing both massspecgym.utils.load_massspecgym() and
# massspecgym.data.datasets.RetrievalDataset.
HF_REPO_ID = "roman-bushuiev/MassSpecGym"
HF_REPO_TYPE = "dataset"

# Pinned commit on the HF dataset repo (fetched via
# `curl https://huggingface.co/api/datasets/roman-bushuiev/MassSpecGym/revision/main`
# on 2026-07-25). Bump deliberately, never silently, and update this comment's date
# when you do.
DATASET_REVISION = "c9aa3feb5f6ec0adee56cc78d2dce24826356156"

# "v1.5" is a data-revision name, independent of the massspecgym pip package's
# own version (pinned separately below). Fetched directly via hf_hub_download at
# the pinned DATASET_REVISION below — not via massspecgym.utils.load_massspecgym(),
# which takes no path argument and always downloads its own unpinned copy of the
# *older* "MassSpecGym.tsv" internally (confirmed against the installed package
# during the real-data preflight run; a previous version of this comment claimed
# otherwise without having actually run it).
DATASET_FILE = "MassSpecGym1.5.tsv"
DATASET_VERSION = "MassSpecGym1.5"

# The "standard" molecular-retrieval candidate pool: mass-filtered, capped at 256
# candidates/query. Confirmed in massspecgym/data/datasets.py::RetrievalDataset.load_data.
CANDIDATES_FILE = "molecules/MassSpecGym1.5_retrieval_candidates_mass.json"
CANDIDATE_POOL = "MassSpecGym1.5_retrieval_candidates_mass.json"

# Official split column and values. Confirmed in massspecgym/data/data_module.py:
# '"Folds" column must contain only "train", "val", or "test" values.'
SPLIT_COLUMN = "fold"
SPLITS = ("train", "val", "test")

# pip-installable massspecgym package version (github.com/pluskal-lab/MassSpecGym
# release tag v1.3.1 == PyPI massspecgym==1.3.1, confirmed via `pip index`/PyPI API
# on 2026-07-25). This versions the *code*, independently of DATASET_VERSION above.
MASSSPECGYM_PACKAGE_VERSION = "1.3.1"

# Simplest official retrieval baseline with a real learned score per candidate
# (simpler than DeepSets/DeepSets+Fourier/MIST per results/retrieval.csv on the
# upstream repo). Confirmed class:
# massspecgym.models.retrieval.fingerprint_ffn.FingerprintFFNRetrieval
MODEL_NAME = "fingerprint_ffn"
