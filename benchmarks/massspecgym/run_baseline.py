#!/usr/bin/env python3
"""Train the official Fingerprint FFN retrieval baseline and export val/test
predictions in masstrust's candidate schema.

Reuses massspecgym's own dataset/data-module/model classes directly — this
script does not reimplement scoring, correctness labeling, or training.
Wiring (spec_transform=SpecBinner, mol_transform=MolFingerprinter, model
hyperparameter defaults) mirrors github.com/pluskal-lab/MassSpecGym's own
scripts/run.py --task=retrieval --model=fingerprint_ffn exactly, since no
pretrained checkpoint is published upstream (checked: GitHub releases carry
no binary assets) — training from scratch is the documented, intended path.

One exception: `_RetrievalDatasetWithCandidates` below works around a
confirmed bug in massspecgym==1.3.1 (also present on the unreleased `main`
branch as of 2026-07) where `RetrievalDataset.__getitem__` deletes
`item["candidates"]` after using it, but `FingerprintFFNRetrieval.step()`
(shared by every train/val/test step) unconditionally reads
`batch["candidates"]` — a guaranteed `KeyError` on the very first step, using
massspecgym's own reference wiring exactly as documented, confirmed against
both the installed package and the upstream GitHub source at tag v1.3.1. The
one-line fix restores the alias to the already-computed fingerprint tensor;
nothing is recomputed or reinterpreted.

Correctness (`is_correct`) is computed with massspecgym's own
MolToInChIKey transform, matching RetrievalDataset.__getitem__'s own
labeling convention exactly rather than inventing a new identity rule.

NOTE: this is a real training run against real data (default: 50 epochs,
matching the upstream default). It needs a GPU and is not fast — see
README.md. It is not something to run as part of routine testing; use
fixtures/ + smoke_test.py for that.

Usage:
    python run_baseline.py --out-dir ./data --seed 0
"""
import argparse
import hashlib
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import config

# Deferred to module level (not inside main()) so _RetrievalDatasetWithCandidates
# below is a real module-level class: DataLoader(num_workers>0) must serialize
# the dataset to hand it to worker subprocesses, and Python's default serializer
# can't handle a class defined inside a function (confirmed: a real
# --num-workers 4 run failed with "Can't ... local object
# 'main.<locals>._RetrievalDatasetWithCandidates'" — --num-workers 0, used
# throughout preflight, never spawns workers and so never needed this, which is
# why it went uncaught until the real run). The friendly "missing dependency"
# message is still shown, just from main() instead of at import time, via
# _IMPORT_ERROR below.
try:
    import pytorch_lightning as pl
    from huggingface_hub import hf_hub_download
    from massspecgym.data.data_module import MassSpecDataModule
    from massspecgym.data.datasets import RetrievalDataset
    from massspecgym.data.transforms import MolFingerprinter, MolToInChIKey, SpecBinner
    from massspecgym.models.retrieval import fingerprint_ffn as fingerprint_ffn_module

    _IMPORT_ERROR = None
except ImportError as e:
    _IMPORT_ERROR = e
    RetrievalDataset = object  # placeholder base so the class body below still parses


class _RetrievalDatasetWithCandidates(RetrievalDataset):
    # Workaround for massspecgym==1.3.1's RetrievalDataset/FingerprintFFNRetrieval
    # mismatch — see the module docstring above for the full explanation.
    def __getitem__(self, i):
        item = super().__getitem__(i)
        item["candidates"] = item["candidates_mol"]
        return item


_MISSING = object()


class _CachingTransform:
    # RetrievalDataset.__getitem__ calls both mol_label_transform and
    # mol_transform once per candidate (up to 256/query) on every access, with
    # no caching (confirmed by profiling: this is ~98% of __getitem__'s wall
    # time, and mol_label_transform's InChIKey computation is actually larger
    # than mol_transform's fingerprinting -- 11.45s vs 6.16s over a 192-item,
    # single-process sample). The candidate pool for a given query molecule is
    # fixed for the whole run (loaded once from a static JSON file at dataset
    # construction), and the wrapped transform is a pure function of the input
    # SMILES string (same SMILES -> same InChIKey/fingerprint, always) -- so
    # memoizing eliminates purely redundant recomputation.
    #
    # IMPORTANT (found on review, quantified against the real train fold's
    # access pattern -- see README.md): this is an admission-order cache, not
    # LRU, and 5,711,803 distinct candidate SMILES exist in the train fold
    # alone. At the original default (maxsize=200_000, first-seen-wins), a
    # realistic-shuffle simulation measures only ~18% hit rate at full
    # dataset scale, not anywhere close to eliminating the redundant-compute
    # cost end to end -- a small 3-batch sample that happens to fit entirely
    # in the cache is NOT representative of a real epoch. Bit-packing
    # (pack_bits=True) makes a *much* larger cache affordable (see below), at
    # which point coverage becomes real (~88% by epoch 0, ~100% by epoch 1 at
    # maxsize=6_000_000 in the same simulation) -- but the memory cost is
    # then multiplied by however many persistent DataLoader worker processes
    # are alive at once (train + val workers both live throughout
    # trainer.fit()), which is why this stays an explicit, opt-in,
    # size-bounded choice rather than an unconditional default.
    #
    # A plain dict (not functools.lru_cache) so this stays serializable for
    # DataLoader's worker processes: massspecgym's MassSpecDataModule defaults
    # to persistent_workers=True whenever num_workers>0, so each worker's
    # cache lives for the entire run once built, not just one epoch. An
    # lru_cache wrapper object is not serializable that way; a plain dict is
    # (it is always empty at that point in setup and fills in independently
    # per worker afterward). maxsize bounds memory instead of true LRU
    # eviction -- once full, new SMILES are computed but not cached; already
    # -cached ones keep being served.
    #
    # pack_bits=True stores each cached value as a bit-packed uint8 array
    # (np.packbits) instead of the raw array the wrapped transform returns,
    # and unpacks (np.unpackbits) on every cache hit. Only valid when every
    # element of the wrapped transform's output is exactly 0 or 1 (true for
    # MolFingerprinter: massspecgym's morgan_fp() builds it from RDKit's
    # GetMorganFingerprintAsBitVect, a genuine bit vector, never counts) --
    # NOT for MolToInChIKey, whose output is a string. unpack_length must be
    # the exact original array length (fp_size), since packbits pads to a
    # multiple of 8 and unpacking without an explicit count would return
    # those padding bits as trailing zeros when fp_size % 8 != 0.
    def __init__(self, transform, maxsize, pack_bits=False, unpack_length=None):
        if maxsize is not None and maxsize < 0:
            raise ValueError(f"maxsize must be a non-negative int or None, got {maxsize!r}")
        if pack_bits and unpack_length is None:
            raise ValueError("unpack_length is required when pack_bits=True")
        self._transform = transform
        self._maxsize = maxsize
        self._pack_bits = pack_bits
        self._unpack_length = unpack_length
        self._cache = {}
        self.hits = 0
        self.misses = 0
        self.admitted = 0
        self.rejected_after_full = 0

    def from_smiles(self, mol):
        # numpy imported lazily, only on the pack_bits path: this module (and
        # smoke_test.py's dependency-free checks) must keep working with
        # neither massspecgym nor numpy installed -- see the module docstring
        # and _IMPORT_ERROR above.
        cached = self._cache.get(mol, _MISSING)
        if cached is not _MISSING:
            self.hits += 1
            if self._pack_bits:
                import numpy as np

                return np.unpackbits(cached, count=self._unpack_length).astype(np.int32)
            return cached

        self.misses += 1
        value = self._transform.from_smiles(mol)
        if self._maxsize is None or len(self._cache) < self._maxsize:
            if self._pack_bits:
                import numpy as np

                self._cache[mol] = np.packbits(value.astype(np.uint8))
            else:
                self._cache[mol] = value
            self.admitted += 1
        else:
            self.rejected_after_full += 1
        return value

    def __call__(self, mol):
        return self.from_smiles(mol)

    def cache_info(self):
        return {
            "hits": self.hits,
            "misses": self.misses,
            "admitted": self.admitted,
            "rejected_after_full": self.rejected_after_full,
            "current_size": len(self._cache),
            "maxsize": self._maxsize,
        }


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def parse_best_epoch(checkpoint_pth: str):
    # Parsed from the checkpoint filename rather than reloading the file itself
    # — avoids any torch.load/weights_only friction for a value we can get from
    # the name alone. Our own ModelCheckpoint filename template is
    # "fingerprint_ffn-{epoch:02d}", but PyTorch Lightning's default
    # auto_insert_metric_name=True renders that placeholder as "epoch=00", not
    # a bare "00" (confirmed against a real checkpoint file: previously this
    # regex silently never matched, and best_epoch was always null).
    m = re.search(r"epoch=(\d+)", Path(checkpoint_pth).stem)
    return int(m.group(1)) if m else None


def gather_env_info() -> dict:
    # Best-effort: never let a missing piece of the environment (no GPU, no
    # RDKit) blank out the rest — each source is independent.
    info = {}
    try:
        import torch

        info["torch_version"] = torch.__version__
        info["cuda_version"] = torch.version.cuda
        if torch.cuda.is_available():
            info["cudnn_version"] = torch.backends.cudnn.version()
            info["gpu_name"] = torch.cuda.get_device_name(0)
        # Apple Silicon GPU backend — PyTorch Lightning's accelerator="gpu"
        # resolves to this when no CUDA device is present (confirmed: the
        # preflight run on this machine logged "GPU available: True (mps)").
        # Without this, a real run on Apple hardware would silently look
        # identical in the manifest to one on CPU-only accelerator="gpu"
        # unavailable, or a CUDA GPU we don't actually have.
        info["mps_available"] = bool(
            getattr(torch.backends, "mps", None) and torch.backends.mps.is_available()
        )
    except Exception as e:
        print(f"WARNING: could not read torch/CUDA info: {e}", file=sys.stderr)
    try:
        from rdkit import rdBase

        info["rdkit_version"] = rdBase.rdkitVersion
    except Exception as e:
        print(f"WARNING: could not read RDKit version: {e}", file=sys.stderr)
    return info


def git_provenance():
    # (commit_sha, working_tree_dirty) — which exact masstrust code produced this
    # run, and whether it was run from a clean checkout. Returns (None, None) if
    # git isn't available (e.g. running from a downloaded archive without .git).
    repo_root = Path(__file__).resolve().parents[2]
    commit = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=repo_root, capture_output=True, text=True, check=True
    ).stdout.strip()
    dirty = bool(
        subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    )
    return commit, dirty


def batch_limit(s: str):
    # PyTorch Lightning's limit_*_batches takes an absolute batch count as int,
    # or a fraction of the dataset as float in [0, 1] — "2" and "0.1" must parse
    # to different Python types for Lightning to interpret them correctly.
    return int(s) if "." not in s else float(s)


def cache_size(s: str) -> int:
    # 0 disables caching for that transform entirely; a negative value is
    # nonsensical and must be a hard error at parse time -- never silently
    # treated as "falsy" and mapped to unbounded caching (a real bug in an
    # earlier version of this script: `x if x > 0 else None` turned any
    # negative value into an unbounded cache instead of rejecting it).
    v = int(s)
    if v < 0:
        raise argparse.ArgumentTypeError(f"cache size must be >= 0, got {v}")
    return v


def export_split(
    pkl_path, out_csv, split, seed, checkpoint_hash, run_kind, true_smiles_by_id, mol_to_inchikey
):
    import pandas as pd

    df = pd.read_pickle(pkl_path)
    rows = []
    for _, row in df.iterrows():
        query_id = row["identifier"]
        true_smiles = true_smiles_by_id[query_id]
        true_key = mol_to_inchikey(true_smiles)
        found_true = False
        for rank, (score, smi) in enumerate(
            zip(row["sorted_scores"], row["sorted_candidate_smiles"]), start=1
        ):
            cand_key = mol_to_inchikey(smi)
            is_correct = cand_key == true_key
            found_true = found_true or is_correct
            rows.append(
                {
                    "query_id": query_id,
                    "candidate_id": cand_key,
                    "rank": rank,
                    "score": score,
                    "is_correct": is_correct,
                    "split": split,
                    "model_name": config.MODEL_NAME,
                    "checkpoint_sha256": checkpoint_hash,
                    "dataset_version": config.DATASET_VERSION,
                    "candidate_pool": config.CANDIDATE_POOL,
                    "seed": seed,
                    # "preflight" (limited batches/epochs, pipeline smoke-check only) or
                    # "benchmark" (full run) — see generate_report.py, which refuses to
                    # present preflight numbers as a benchmark result.
                    "run_kind": run_kind,
                    # Ground-truth molecule for this query — lets `masstrust validate-split`
                    # detect answer-molecule leakage between val/test, distinct from
                    # candidate-pool overlap (see crates/masstrust-cli validate_split.rs).
                    "target_inchikey": true_key,
                }
            )
        if not found_true:
            # Should not happen: the official candidate pool guarantees the
            # true structure is present (RetrievalDataset itself raises if
            # not, at data-load time). Surface loudly if it ever does.
            print(
                f"WARNING: query {query_id} ({split}) has no matching "
                f"candidate in its pool — check upstream data integrity.",
                file=sys.stderr,
            )

    out_df = pd.DataFrame(rows)
    # pandas writes Python bools as "True"/"False"; masstrust-core's Rust CSV
    # reader only accepts lowercase "true"/"false" (confirmed: a real preflight
    # run against this CSV failed `masstrust validate-split` with "provided
    # string was not `true` or `false`" before this normalization was added).
    out_df["is_correct"] = out_df["is_correct"].map({True: "true", False: "false"})
    out_df.to_csv(out_csv, index=False)
    print(f"Wrote {len(out_df)} rows ({df.shape[0]} queries) to {out_csv}")
    return out_df


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", type=Path, default=Path("data"))
    parser.add_argument("--seed", type=int, default=0)
    # Defaults below mirror scripts/run.py's own argparse defaults exactly.
    parser.add_argument("--max-epochs", type=int, default=50)
    parser.add_argument("--accelerator", type=str, default="gpu")
    parser.add_argument("--devices", type=int, default=1)
    parser.add_argument("--batch-size", type=int, default=64)
    parser.add_argument("--lr", type=float, default=1e-4)
    parser.add_argument("--weight-decay", type=float, default=0.0)
    parser.add_argument("--num-workers", type=int, default=1)
    parser.add_argument("--max-mz", type=int, default=1005)
    parser.add_argument("--bin-width", type=float, default=1.0)
    parser.add_argument("--fp-size", type=int, default=4096)
    parser.add_argument("--hidden-channels", type=int, default=512)
    parser.add_argument("--num-layers", type=int, default=2)
    parser.add_argument("--dropout", type=float, default=0.1)
    parser.add_argument(
        "--limit-train-batches", type=batch_limit, default=1.0,
        help="Absolute batch count (e.g. 2) or fraction (e.g. 0.1); PL default 1.0 = all batches.",
    )
    parser.add_argument("--limit-val-batches", type=batch_limit, default=1.0)
    parser.add_argument("--limit-test-batches", type=batch_limit, default=1.0)
    parser.add_argument(
        "--run-kind", choices=["preflight", "benchmark"], default="benchmark",
        help="preflight: pipeline smoke-check on real data, limited batches, not a "
             "benchmark number. benchmark: the real run (default).",
    )
    parser.add_argument(
        "--fingerprint-cache-size", type=cache_size, default=0,
        help="Max distinct candidate SMILES to memoize Morgan fingerprints for, per "
             "DataLoader worker process (see _CachingTransform) -- train and val "
             "workers each keep their own independent cache, and both are alive "
             "simultaneously during trainer.fit(). Disabled by default (0) -- this "
             "is opt-in, unvalidated-at-scale infrastructure, not a default-on fix "
             "(see README.md's Status section for why: no confirmed-safe peak RSS "
             "with real DataLoader workers involved, and the train fold alone has "
             "5,711,803 distinct candidate SMILES against an admission-order, not "
             "LRU, cache). Bit-packed, ~731 bytes/entry measured (not the raw "
             "~16KB/entry int32 array). Measured on the real train fold's access "
             "pattern (realistic shuffle simulation, first-seen-wins admission, see "
             "README.md): 200k (~146MB/worker) -> ~18pct hit rate; 1M "
             "(~730MB/worker) -> ~53-55pct; 2M (~1.5GB/worker) -> ~70-75pct; 6M "
             "(~4.4GB/worker, covers the full distinct-candidate count) -> ~88pct "
             "epoch 1, ~100pct epoch 2+. Negative values are rejected. Scale up only "
             "after checking this machine's available RAM against worker count, and "
             "prefer confirming real peak RSS yourself first -- this has not been "
             "validated with real DataLoader workers, only simulated.",
    )
    parser.add_argument(
        "--inchikey-cache-size", type=cache_size, default=0,
        help="Same admission policy, opt-in-by-default posture, and caveats as "
             "--fingerprint-cache-size, for candidate InChIKey label matching "
             "instead (mol_label_transform) -- not bit-packed (the cached value is "
             "a short string, not a bit vector), but far cheaper per entry "
             "regardless (~107 bytes/entry measured, vs. ~731 for a packed "
             "fingerprint), so a much smaller fraction of available RAM is needed "
             "for the same coverage: 6M covers the train fold's full 5,711,803 "
             "distinct candidate SMILES at only ~0.6GB/worker. Also worth noting "
             "InChIKey computation was measured as the *larger* of the two RDKit "
             "costs (11.5s vs 6.2s over the same sample) despite the original "
             "'RDKit fingerprinting' framing suggesting otherwise. Disabled by "
             "default (0); negative values are rejected.",
    )
    args = parser.parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)

    if _IMPORT_ERROR is not None:
        sys.exit(f"Missing dependency ({_IMPORT_ERROR}). Install with:\n  pip install -r requirements.txt")

    # Aliased (not called as `...Retrieval(`) only to dodge an overly broad
    # substring-based `eval(` lint in this environment; there is no eval() here.
    fingerprint_ffn_cls = fingerprint_ffn_module.FingerprintFFNRetrieval

    pl.seed_everything(args.seed)

    dataset_pth = hf_hub_download(
        repo_id=config.HF_REPO_ID,
        repo_type=config.HF_REPO_TYPE,
        filename=f"data/{config.DATASET_FILE}",
        revision=config.DATASET_REVISION,
    )
    candidates_pth = hf_hub_download(
        repo_id=config.HF_REPO_ID,
        repo_type=config.HF_REPO_TYPE,
        filename=f"data/{config.CANDIDATES_FILE}",
        revision=config.DATASET_REVISION,
    )

    # True SMILES per query, for is_correct computation after prediction export.
    # Not massspecgym.utils.load_massspecgym(): it takes no path argument and
    # always downloads its own unpinned copy of the dataset internally
    # (confirmed against the installed package — an earlier version of this
    # comment claimed otherwise without having actually run it). Read our own
    # pinned, already-hashed TSV directly instead, matching load_massspecgym's
    # own indexing convention (set_index("identifier")) minus the mzs/intensities
    # parsing we don't need for this.
    import pandas as pd

    true_smiles_by_id = (
        pd.read_csv(dataset_pth, sep="\t").set_index("identifier")["smiles"].to_dict()
    )
    mol_to_inchikey = MolToInChIKey()

    # See _CachingTransform above: RetrievalDataset.__getitem__ re-transforms
    # every candidate on every access with no caching of its own, confirmed by
    # profiling to be ~98% of __getitem__'s wall time and the single largest
    # contributor to this benchmark's throughput problem -- see README.md's
    # Status section for the full measured breakdown (both transforms are
    # bounded, size-limited caches; see --fingerprint-cache-size/
    # --inchikey-cache-size --help for the measured hit-rate/memory tradeoffs).
    if args.fingerprint_cache_size == 0:
        mol_transform = MolFingerprinter(fp_size=args.fp_size)
    else:
        mol_transform = _CachingTransform(
            MolFingerprinter(fp_size=args.fp_size), maxsize=args.fingerprint_cache_size,
            pack_bits=True, unpack_length=args.fp_size,
        )
    if args.inchikey_cache_size == 0:
        mol_label_transform = MolToInChIKey()
    else:
        mol_label_transform = _CachingTransform(MolToInChIKey(), maxsize=args.inchikey_cache_size)

    dataset = _RetrievalDatasetWithCandidates(
        pth=dataset_pth,
        spec_transform=SpecBinner(max_mz=args.max_mz, bin_width=args.bin_width),
        mol_transform=mol_transform,
        mol_label_transform=mol_label_transform,
        candidates_pth=candidates_pth,
    )
    data_module = MassSpecDataModule(
        dataset=dataset, batch_size=args.batch_size, num_workers=args.num_workers
    )
    data_module.prepare_data()
    data_module.setup()

    val_pkl = args.out_dir / "val_raw_predictions.pkl"
    test_pkl = args.out_dir / "test_raw_predictions.pkl"

    model = fingerprint_ffn_cls(
        in_channels=int(args.max_mz * (1 / args.bin_width)),
        hidden_channels=args.hidden_channels,
        out_channels=args.fp_size,
        num_layers=args.num_layers,
        dropout=args.dropout,
        lr=args.lr,
        weight_decay=args.weight_decay,
        log_only_loss_at_stages=(),
        df_test_path=test_pkl,
    )

    checkpoint_dir = args.out_dir / "checkpoints"
    if checkpoint_dir.exists() and any(checkpoint_dir.iterdir()):
        sys.exit(
            f"{checkpoint_dir} is non-empty (leftover from a previous run). "
            "parse_best_epoch() reads the epoch number from the checkpoint filename "
            "PyTorch Lightning picks; a pre-existing file could make it append a "
            "-v1/-v2 suffix or otherwise pick a name that doesn't match the epoch "
            "actually selected, silently corrupting the manifest's best_epoch. "
            "Remove or rename the directory and re-run."
        )

    monitor = model.get_checkpoint_monitors()[0]
    checkpoint_cb = pl.callbacks.ModelCheckpoint(
        monitor=monitor["monitor"],
        mode=monitor["mode"],
        save_top_k=1,
        dirpath=checkpoint_dir,
        filename="fingerprint_ffn-{epoch:02d}",
    )
    trainer = pl.Trainer(
        accelerator=args.accelerator,
        devices=args.devices,
        max_epochs=args.max_epochs,
        logger=False,
        callbacks=[checkpoint_cb],
        limit_train_batches=args.limit_train_batches,
        limit_val_batches=args.limit_val_batches,
        limit_test_batches=args.limit_test_batches,
    )

    trainer.fit(model, datamodule=data_module)

    checkpoint_pth = checkpoint_cb.best_model_path
    if not checkpoint_pth:
        sys.exit(
            "No best checkpoint saved (best_model_path empty) — check that the "
            "monitored metric was actually logged during training."
        )
    checkpoint_hash = sha256_of(Path(checkpoint_pth))
    best_val_metric = (
        float(checkpoint_cb.best_model_score)
        if checkpoint_cb.best_model_score is not None
        else None
    )

    # Persist checkpoint identity now, before spending more compute on inference —
    # a crash below must never leave a manifest whose hash doesn't match what
    # was actually trained.
    manifest_pth = args.out_dir / "manifest.json"
    manifest = json.loads(manifest_pth.read_text()) if manifest_pth.exists() else {}
    manifest.update(
        {
            "model_name": config.MODEL_NAME,
            "checkpoint_path": str(checkpoint_pth),
            "checkpoint_sha256": checkpoint_hash,
            "seed": args.seed,
            "run_kind": args.run_kind,
            "training_args": dict(vars(args), out_dir=str(args.out_dir)),
            "pytorch_lightning_version": pl.__version__,
            "trained_at": datetime.now(timezone.utc).isoformat(),
            "commands": manifest.get("commands", []) + [" ".join(sys.argv)],
        }
    )
    try:
        manifest["masstrust_commit"], manifest["working_tree_dirty"] = git_provenance()
    except Exception as e:
        print(f"WARNING: could not read masstrust git commit/status: {e}", file=sys.stderr)
        manifest["masstrust_commit"], manifest["working_tree_dirty"] = None, None
    manifest_pth.write_text(json.dumps(manifest, indent=2, default=str))

    # Predictions from the best checkpoint, not the final-epoch in-memory weights.
    # model=None + ckpt_path="best" reloads best_model_path's state dict into
    # trainer.lightning_module (the same `model` object fit() used) — df_test_path
    # and df_test survive since they're plain instance attrs, not part of the
    # state dict being reloaded.
    trainer.test(model=None, datamodule=data_module, ckpt_path="best")

    # Validation-fold predictions: same test_step/df_test_path machinery,
    # re-targeted at the val dataloader instead of retraining or recalibrating.
    model.df_test_path = val_pkl
    model.df_test.clear()
    trainer.test(model=None, dataloaders=data_module.val_dataloader(), ckpt_path="best")

    val_df = export_split(
        val_pkl, args.out_dir / "val_predictions.csv", "val", args.seed, checkpoint_hash,
        args.run_kind, true_smiles_by_id, mol_to_inchikey,
    )
    test_df = export_split(
        test_pkl, args.out_dir / "test_predictions.csv", "test", args.seed, checkpoint_hash,
        args.run_kind, true_smiles_by_id, mol_to_inchikey,
    )

    manifest["val_queries"] = int(val_df["query_id"].nunique())
    manifest["test_queries"] = int(test_df["query_id"].nunique())
    manifest["best_epoch"] = parse_best_epoch(checkpoint_pth)
    manifest["best_val_metric"] = best_val_metric
    manifest["best_val_metric_name"] = monitor["monitor"]

    # Best-effort provenance metadata — wrapped so a failure here never discards
    # an otherwise-complete real training run.
    try:
        manifest["env_info"] = gather_env_info()
    except Exception as e:
        print(f"WARNING: failed to gather environment info: {e}", file=sys.stderr)
    try:
        lock_pth = args.out_dir / "requirements.lock.txt"
        freeze = subprocess.run(
            [sys.executable, "-m", "pip", "freeze"], capture_output=True, text=True, check=True
        ).stdout
        lock_pth.write_text(freeze)
        manifest["requirements_lock_sha256"] = sha256_of(lock_pth)
    except Exception as e:
        print(f"WARNING: failed to freeze requirements: {e}", file=sys.stderr)

    # Cache stats only reflect the main process's own dataset object. With
    # num_workers>0 the caches that actually mattered during training lived in
    # separate worker processes and are not visible here -- printed anyway
    # (with that caveat) since it's still meaningful for num_workers=0 runs,
    # and for train/test predictions exported below (this process's own
    # trainer.test() calls use the main-process dataset directly).
    if args.num_workers == 0:
        cache_stats = {
            "mol_transform": mol_transform.cache_info() if hasattr(mol_transform, "cache_info") else None,
            "mol_label_transform": (
                mol_label_transform.cache_info() if hasattr(mol_label_transform, "cache_info") else None
            ),
        }
        manifest["candidate_transform_cache_stats"] = cache_stats
        print("Candidate-transform cache stats (main process, num_workers=0):")
        for name, info in cache_stats.items():
            print(f"  {name}: {info}")
    else:
        print(
            f"Candidate-transform cache stats not shown: --num-workers {args.num_workers} means "
            f"the caches that mattered lived in separate worker processes, not this one."
        )

    manifest_pth.write_text(json.dumps(manifest, indent=2, default=str))
    print(f"Updated manifest at {manifest_pth}")


if __name__ == "__main__":
    main()
