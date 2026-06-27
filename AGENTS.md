# AGENTS.md

## Project: masstrust

`masstrust` is a Rust toolkit for selective prediction, confidence calibration, and abstention in MS/MS molecular annotation.

The project does **not** perform molecular structure prediction by itself. Instead, it takes candidate rankings produced by external MS/MS annotation or retrieval tools and decides whether the top annotation should be trusted or rejected under a user-specified error-rate target.

Core idea:

> Trust confident annotations. Abstain when uncertainty is too high.

This project is intended for clinical metabolomics, environmental screening, and other MS/MS annotation workflows where false annotations can be costly.

---

## Product Positioning

`masstrust` is a post-hoc trust layer for MS/MS molecular annotations.

It should sit after tools such as:

* SIRIUS / CSI:FingerID
* MSNovelist-style models
* MassSpecGym retrieval models
* in-house candidate rankers
* any system that outputs ranked molecular candidates with scores

It should not try to replace those systems.

The main responsibility is:

1. Accept candidate rankings.
2. Compute confidence scores.
3. Estimate risk-coverage tradeoffs.
4. Select thresholds for a target error rate.
5. Abstain from unreliable annotations.
6. Export reproducible policies and evaluation artifacts.

---

## Non-Goals

Do **not** turn this project into a general cheminformatics toolkit.

Do **not** implement a full MS/MS search engine.

Do **not** implement molecular structure generation.

Do **not** implement retrosynthesis.

Do **not** make `masstrust-core` depend on heavy chemistry or ML libraries by default.

Do **not** make scientific claims that are not backed by tests, benchmarks, examples, or cited references.

Do **not** claim that `masstrust` guarantees clinical correctness. It only provides calibrated post-hoc decision support based on available validation data.

---

## Relationship to Other Repositories

### chematic

`chematic` may be used optionally for molecule normalization, fingerprints, descriptors, or candidate ambiguity features.

However, `masstrust-core` must remain independent from `chematic` by default.

If integration is added, it should be behind a feature flag:

```toml
[features]
chematic = ["dep:chematic"]
```

### renkin

`renkin` is a retrosynthesis engine and is not directly related to this project.

Do not add retrosynthesis-specific logic to `masstrust`.

---

## Recommended Repository Structure

Use a Rust workspace from the beginning.

```text
masstrust/
  AGENTS.md
  Cargo.toml
  README.md
  LICENSE
  crates/
    masstrust-core/
      Cargo.toml
      src/
        lib.rs
        types.rs
        error.rs
        scoring/
          mod.rs
          max_prob.rs
          score_gap.rs
          margin.rs
          entropy.rs
        metrics/
          mod.rs
          risk_coverage.rs
          aurc.rs
        calibration/
          mod.rs
          empirical.rs
          binomial.rs
        policy.rs
        io.rs
    masstrust-cli/
      Cargo.toml
      src/
        main.rs
        commands/
          mod.rs
          curve.rs
          calibrate.rs
          apply.rs
  examples/
    candidates.csv
    labeled_candidates.csv
  tests/
  benches/
  docs/
```

Optional future crates:

```text
crates/
  masstrust-py/        # Python bindings via pyo3
  masstrust-plot/      # optional plotting helpers
```

---

## Core Concepts

### Candidate

A candidate is one possible molecular annotation for a query spectrum.

Required fields:

* `query_id`
* `candidate_id`
* `rank`
* `score`

Optional fields:

* `probability`
* `smiles`
* `inchikey`
* `formula`
* `is_correct`

### Query-Level Decision

For each query spectrum, `masstrust` usually decides whether to accept or abstain from the top-ranked candidate.

A decision contains:

* `query_id`
* `selected_candidate_id`
* `confidence`
* `accepted`
* `threshold`
* `method`

### Risk

Risk is the error rate among accepted annotations.

For top-1 annotation evaluation:

```text
risk = incorrect_accepted / accepted
```

Coverage is:

```text
coverage = accepted / total_queries
```

The main evaluation artifact is the risk-coverage curve.

---

## Initial Public API Sketch

Prefer simple, explicit types.

```rust
pub struct Candidate {
    pub query_id: String,
    pub candidate_id: String,
    pub rank: usize,
    pub score: f64,
    pub probability: Option<f64>,
    pub smiles: Option<String>,
    pub inchikey: Option<String>,
    pub formula: Option<String>,
    pub is_correct: Option<bool>,
}

pub struct QueryRanking {
    pub query_id: String,
    pub candidates: Vec<Candidate>,
}

pub struct AnnotationDecision {
    pub query_id: String,
    pub selected_candidate_id: String,
    pub confidence: f64,
    pub accepted: bool,
    pub threshold: f64,
    pub method: String,
}

pub struct TrustPolicy {
    pub scoring_method: ScoringMethod,
    pub threshold: f64,
    pub target_error_rate: f64,
    pub calibration_method: CalibrationMethod,
}
```

Use `thiserror` for errors.

Use `serde` for serialization.

Use deterministic behavior wherever possible.

---

## MVP Scope

The first MVP should include:

1. CSV input parser for candidate rankings.
2. Grouping by `query_id`.
3. Top-1 candidate extraction.
4. Confidence scoring:

   * `max_prob`
   * `score_gap`
   * `margin`
   * `entropy`
5. Risk-coverage curve generation.
6. Empirical threshold calibration.
7. Conservative binomial threshold calibration.
8. Policy export/import as JSON.
9. CLI commands:

   * `curve`
   * `calibrate`
   * `apply`
10. Unit tests and integration tests.

Do not add ML training in the MVP.

Do not add Python bindings in the MVP unless the Rust core and CLI are already stable.

---

## CLI Design

The CLI binary should be named:

```text
masstrust
```

Target commands:

```bash
masstrust curve labeled_candidates.csv \
  --score score-gap \
  --out risk_coverage.csv

masstrust calibrate labeled_candidates.csv \
  --score score-gap \
  --error-rate 0.05 \
  --method empirical \
  --out policy.json

masstrust calibrate labeled_candidates.csv \
  --score score-gap \
  --error-rate 0.05 \
  --method binomial \
  --confidence-level 0.95 \
  --out policy.json

masstrust apply candidates.csv \
  --policy policy.json \
  --out trusted_annotations.csv \
  --abstained abstained.csv
```

CLI behavior must be deterministic and script-friendly.

Prefer explicit errors over silent fallback.

---

## Input CSV Format

The MVP should support this minimal format:

```csv
query_id,candidate_id,rank,score,probability,is_correct
q1,cand_a,1,0.92,0.71,true
q1,cand_b,2,0.81,0.21,false
q1,cand_c,3,0.70,0.08,false
q2,cand_d,1,0.88,0.46,false
q2,cand_e,2,0.86,0.43,true
```

Required columns:

* `query_id`
* `candidate_id`
* `rank`
* `score`

Required for evaluation/calibration:

* `is_correct`

Optional:

* `probability`
* `smiles`
* `inchikey`
* `formula`

The parser should produce helpful error messages when required columns are missing.

---

## Confidence Scoring Methods

### max_prob

Use the top-1 candidate probability.

Requires `probability`.

### score_gap

Use:

```text
score(top1) - score(top2)
```

If only one candidate exists, define behavior explicitly. Prefer returning an error or using a documented fallback.

### margin

Use:

```text
probability(top1) - probability(top2)
```

Requires probabilities for at least two candidates.

### entropy

Compute entropy over candidate probabilities.

Lower entropy means higher confidence.

For consistency, convert entropy to confidence so that higher confidence is always better.

Example:

```text
confidence = 1.0 - normalized_entropy
```

Document the normalization.

---

## Risk-Coverage Curve

A risk-coverage curve should be computed by sorting queries by confidence descending.

For each threshold or prefix:

* accepted count
* coverage
* error count
* risk

Output CSV columns:

```csv
threshold,accepted,total,coverage,errors,risk
```

Risk must be undefined or represented carefully when `accepted = 0`.

Avoid division-by-zero.

---

## Threshold Calibration

### Empirical Calibration

Select the threshold that maximizes coverage while keeping observed risk less than or equal to the target error rate.

Example:

```text
target_error_rate = 0.05
choose threshold with max coverage where observed_risk <= 0.05
```

### Conservative Binomial Calibration

Use a one-sided binomial upper confidence bound for the error rate among accepted predictions.

Select the threshold that maximizes coverage while keeping the upper bound less than or equal to the target error rate.

This mode is more conservative and should be recommended for high-stakes settings.

Do not overstate guarantees.

---

## Policy JSON

A saved policy should include enough information to reproduce decisions.

Example:

```json
{
  "version": "0.1.0",
  "scoring_method": "score_gap",
  "threshold": 0.18,
  "target_error_rate": 0.05,
  "calibration_method": "binomial",
  "confidence_level": 0.95,
  "created_by": "masstrust"
}
```

Policy loading must validate:

* version
* scoring method
* threshold
* calibration method

Unknown fields should not crash unless strict mode is enabled.

---

## Testing Requirements

Every meaningful feature must have tests.

Minimum tests:

* Candidate CSV parsing.
* Grouping candidates by query.
* Top-1 extraction.
* Each confidence scoring method.
* Risk-coverage curve calculation.
* Empirical threshold selection.
* Binomial threshold selection.
* Policy JSON roundtrip.
* CLI smoke tests.

Use small deterministic fixtures.

Add regression tests for edge cases:

* empty input
* one candidate only
* missing required columns
* duplicate ranks
* tied scores
* missing probability
* no accepted candidates
* all correct
* all incorrect
* NaN / infinite scores
* non-monotonic input ordering

Do not ignore NaN handling. Define and test it.

---

## Coding Standards

Use idiomatic Rust.

Prefer:

* clear data types
* small modules
* explicit errors
* deterministic sorting
* zero panics in library code
* helpful CLI diagnostics

Avoid:

* hidden global state
* unsafe code
* unbounded memory growth
* over-generic abstractions
* premature optimization
* silently swallowing invalid input

Library code should return `Result<T, MasstrustError>`.

CLI code should convert errors into readable messages.

---

## Dependencies

Keep dependencies minimal.

Recommended:

```toml
serde
serde_json
csv
thiserror
clap
anyhow
```

Optional later:

```toml
statrs        # for binomial intervals if needed
plotters      # for SVG risk-coverage plots
pyo3          # Python bindings
polars        # large table support, behind feature flag only
```

Avoid heavy dependencies in `masstrust-core`.

---

## CI Requirements

Set up GitHub Actions early.

Minimum CI checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

Recommended security checks:

```bash
cargo deny check
cargo audit
```

Do not merge code that fails formatting, clippy, tests, or docs.

---

## Documentation Requirements

README should explain:

1. What `masstrust` does.
2. What it does not do.
3. Why abstention matters.
4. Input CSV format.
5. Basic CLI examples.
6. How to interpret risk and coverage.
7. How to choose a threshold.
8. Limitations.

Avoid exaggerated claims.

Good tagline:

```text
Calibrated trust and abstention for MS/MS molecular annotations.
```

Good short description:

```text
masstrust is a Rust toolkit for selective prediction and confidence calibration in MS/MS molecular annotation.
```

---

## Scientific Communication Rules

Be precise.

Use terms consistently:

* candidate ranking
* confidence score
* risk
* coverage
* threshold
* calibration set
* abstention
* accepted annotation

Do not use “accuracy guarantee” casually.

Prefer:

```text
calibrated on validation data
```

or:

```text
controls observed or bounded risk under the chosen calibration procedure
```

Do not claim clinical validity.

Do not claim regulatory compliance.

Do not imply that abstention eliminates all false annotations.

---

## Development Roadmap

### v0.1.0

Goal: usable post-hoc CLI and Rust library.

Features:

* CSV input
* confidence scoring
* risk-coverage curve
* empirical calibration
* binomial calibration
* policy JSON
* apply policy to new rankings

### v0.2.0

Goal: better evaluation and reporting.

Features:

* SVG/PNG risk-coverage plots
* AURC metric
* E-AURC metric if appropriate
* confidence histograms
* richer CLI reports
* MassSpecGym-compatible example data format

### v0.3.0

Goal: ecosystem integration.

Features:

* optional `chematic` integration
* optional Python bindings
* parquet support behind feature flag
* batch processing for large datasets

### Implemented (experimental)

* CRC-style calibration (`--method crc`) — empirical target tightened by `1/(n+1)`
  * Assumes i.i.d. calibration data and binary 0/1 annotation loss
  * Benchmark against empirical / binomial pending
  * Expressed as `experimental` in docs; do not use `guaranteed` language

* Grouped calibration (`--group-col <column>`)
  * Per-group thresholds stored in policy JSON (`group_col`, `group_thresholds`)
  * Queries with unknown group fall back to global threshold
  * Requires sufficient labeled data per group for calibration
  * Expressed as a feature, not a statistical guarantee

### Future

Possible advanced features:

* Validated CRC examples on public MS/MS benchmarks (MassSpecGym)
* Monotone loss formulation beyond binary 0/1
* Grouped calibration: additional examples, docs, compound-class support
* Per-dataset calibration reports
* Uncertainty ensembles
* Bootstrap confidence intervals for AURC
* Calibration drift detection

Do not implement future features before the MVP is stable.

---

## Branch Strategy

Use focused feature branches.

Recommended branches:

```text
main
develop
feat/core-types
feat/scoring
feat/risk-coverage
feat/calibration
feat/cli
feat/policy-json
feat/docs
```

`main` should always be release-ready.

Use pull requests for meaningful changes.

Prefer small PRs with tests.

---

## Agent Workflow

When an AI agent works on this repository:

1. Read `AGENTS.md`.
2. Read `README.md`.
3. Inspect `Cargo.toml` and crate structure.
4. Identify the smallest useful change.
5. Implement the change.
6. Add or update tests.
7. Run formatting.
8. Run clippy.
9. Run tests.
10. Summarize what changed and what remains.

Never make broad rewrites without a clear reason.

Never add unrelated features.

Never change public APIs without updating docs and tests.

---

## Definition of Done

A task is done only when:

* code compiles
* tests pass
* formatting passes
* clippy passes
* docs are updated if behavior changed
* edge cases are considered
* CLI examples still work
* no unrelated changes are included

---

## Preferred Implementation Order

Start here:

1. Create workspace.
2. Create `masstrust-core`.
3. Add `Candidate`, `QueryRanking`, `AnnotationDecision`, `TrustPolicy`.
4. Implement CSV parsing.
5. Implement grouping by `query_id`.
6. Implement `score_gap`.
7. Implement risk-coverage calculation.
8. Implement empirical calibration.
9. Create `masstrust-cli`.
10. Add `curve` command.
11. Add `calibrate` command.
12. Add `apply` command.
13. Add README examples.
14. Add CI.

Keep the first version boring, correct, and well-tested.

---

## Important Design Principle

`masstrust` should be useful even if the user does not care about Rust.

The CLI should be simple enough for scientists and analysts to use from shell scripts.

The Rust library should be clean enough for developers to embed in larger MS/MS pipelines.

When in doubt, prefer clarity, reproducibility, and conservative claims.
