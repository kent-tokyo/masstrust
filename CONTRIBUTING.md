# Contributing to masstrust

Thank you for your interest in contributing.

## Development setup

```bash
git clone https://github.com/kent-tokyo/masstrust
cd masstrust

# Build and test
cargo build --workspace --exclude masstrust-py
cargo test  --workspace --exclude masstrust-py
cargo clippy --workspace --exclude masstrust-py --all-targets -- -D warnings
cargo fmt --all -- --check

# Python wheel (requires maturin)
maturin build --features extension-module
pip install target/wheels/masstrust-*.whl
```

## Definition of done

A change is complete only when:

- `cargo fmt --all` passes
- `cargo clippy --workspace --exclude masstrust-py --all-targets -- -D warnings` passes
- `cargo test --workspace --exclude masstrust-py` passes
- `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --exclude masstrust-py --no-deps` passes
- Edge cases and error paths are tested

## Scientific claims

Before adding or strengthening a statistical claim in docs or code:

- State the assumed data-generating model explicitly
- Cite a primary source
- Name any conditions under which the claim fails
- Prefer "controls observed risk" over "guarantees accuracy"

## Non-goals

Do not add:

- MS/MS search or spectral matching logic
- Molecular structure generation or retrosynthesis
- Heavy chemistry or ML dependencies to `masstrust-core` by default

See `AGENTS.md` for the full non-goals list.

## Pull requests

- Keep PRs focused and small
- Add or update tests for every changed behaviour
- Update `CHANGELOG.md` under `[Unreleased]`
- Reference relevant issues in the PR description
