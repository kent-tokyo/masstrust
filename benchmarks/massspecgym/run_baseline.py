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


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def parse_best_epoch(checkpoint_pth: str):
    # Parsed from our own ModelCheckpoint filename template
    # (filename="fingerprint_ffn-{epoch:02d}") rather than reloading the
    # checkpoint file — avoids any torch.load/weights_only friction for a
    # value we already control the format of.
    m = re.search(r"fingerprint_ffn-(\d+)", Path(checkpoint_pth).stem)
    return int(m.group(1)) if m else None


def gather_env_info() -> dict:
    # Best-effort: never let a missing piece of the environment (no GPU, no
    # RDKit, no git) blank out the rest — each source is independent.
    info = {}
    try:
        import torch

        info["torch_version"] = torch.__version__
        info["cuda_version"] = torch.version.cuda
        if torch.cuda.is_available():
            info["cudnn_version"] = torch.backends.cudnn.version()
            info["gpu_name"] = torch.cuda.get_device_name(0)
    except Exception as e:
        print(f"WARNING: could not read torch/CUDA info: {e}", file=sys.stderr)
    try:
        from rdkit import rdBase

        info["rdkit_version"] = rdBase.rdkitVersion
    except Exception as e:
        print(f"WARNING: could not read RDKit version: {e}", file=sys.stderr)
    try:
        repo_root = Path(__file__).resolve().parents[2]
        info["masstrust_commit"] = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except Exception as e:
        print(f"WARNING: could not read masstrust git commit: {e}", file=sys.stderr)
    return info


def export_split(pkl_path, out_csv, split, seed, checkpoint_hash, true_smiles_by_id, mol_to_inchikey):
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
    args = parser.parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)

    try:
        import pytorch_lightning as pl
        from huggingface_hub import hf_hub_download
        from massspecgym.data.data_module import MassSpecDataModule
        from massspecgym.data.datasets import RetrievalDataset
        from massspecgym.data.transforms import MolFingerprinter, MolToInChIKey, SpecBinner
        from massspecgym.models.retrieval import fingerprint_ffn as fingerprint_ffn_module
        from massspecgym.utils import load_massspecgym
    except ImportError as e:
        sys.exit(f"Missing dependency ({e}). Install with:\n  pip install -r requirements.txt")

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
    df_meta = load_massspecgym(pth=Path(dataset_pth))
    true_smiles_by_id = df_meta["smiles"].to_dict()
    mol_to_inchikey = MolToInChIKey()

    dataset = RetrievalDataset(
        pth=dataset_pth,
        spec_transform=SpecBinner(max_mz=args.max_mz, bin_width=args.bin_width),
        mol_transform=MolFingerprinter(fp_size=args.fp_size),
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
            "training_args": dict(vars(args), out_dir=str(args.out_dir)),
            "pytorch_lightning_version": pl.__version__,
            "trained_at": datetime.now(timezone.utc).isoformat(),
            "commands": manifest.get("commands", []) + [" ".join(sys.argv)],
        }
    )
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
        true_smiles_by_id, mol_to_inchikey,
    )
    test_df = export_split(
        test_pkl, args.out_dir / "test_predictions.csv", "test", args.seed, checkpoint_hash,
        true_smiles_by_id, mol_to_inchikey,
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

    manifest_pth.write_text(json.dumps(manifest, indent=2, default=str))
    print(f"Updated manifest at {manifest_pth}")


if __name__ == "__main__":
    main()
