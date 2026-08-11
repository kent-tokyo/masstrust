use std::collections::HashMap;
use std::path::Path;

use crate::error::MasstrustError;
use crate::types::{Candidate, PolicyFile, QueryRanking};

const REQUIRED_COLUMNS: &[&str] = &["query_id", "candidate_id", "rank", "score"];
const POLICY_VERSION: &str = "0.1.0";

/// Read candidates from `path`.
///
/// Accepts **CSV** (any extension other than `.parquet`) or, when the crate is compiled with
/// the `parquet` feature, **Parquet** files (auto-detected by `.parquet` extension).
///
/// Required CSV columns: `query_id`, `candidate_id`, `rank`, `score`.
/// Returns [`MasstrustError::MissingColumn`] if any required column is absent.
pub fn read_candidates(path: &Path) -> Result<Vec<Candidate>, MasstrustError> {
    if path.extension().is_some_and(|e| e == "parquet") {
        #[cfg(feature = "parquet")]
        return read_candidates_parquet(path);
        #[cfg(not(feature = "parquet"))]
        return Err(MasstrustError::ParquetNotEnabled);
    }
    read_candidates_csv(path)
}

fn read_candidates_csv(path: &Path) -> Result<Vec<Candidate>, MasstrustError> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers = rdr.headers()?.clone();

    for col in REQUIRED_COLUMNS {
        if !headers.iter().any(|h| h == *col) {
            return Err(MasstrustError::MissingColumn(col.to_string()));
        }
    }

    let candidates: Result<Vec<Candidate>, _> = rdr.deserialize().collect();
    Ok(candidates?)
}

#[cfg(feature = "parquet")]
fn read_candidates_parquet(path: &Path) -> Result<Vec<Candidate>, MasstrustError> {
    use polars::prelude::*;

    let df = LazyFrame::scan_parquet(path, ScanArgsParquet::default())
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?
        .collect()
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?;

    macro_rules! req_str {
        ($name:expr) => {
            df.column($name)
                .map_err(|_| MasstrustError::MissingColumn($name.to_string()))?
                .str()
                .map_err(|e| MasstrustError::Parquet(e.to_string()))?
                .clone()
        };
    }
    macro_rules! opt_str {
        ($name:expr) => {
            df.column($name)
                .ok()
                .and_then(|s| s.str().ok().map(|ca| ca.clone()))
        };
    }
    macro_rules! opt_f64 {
        ($name:expr) => {
            df.column($name)
                .ok()
                .and_then(|s| s.cast(&DataType::Float64).ok())
                .and_then(|s| s.f64().ok().map(|ca| ca.clone()))
        };
    }

    let query_ids = req_str!("query_id");
    let candidate_ids = req_str!("candidate_id");
    let ranks = df
        .column("rank")
        .map_err(|_| MasstrustError::MissingColumn("rank".into()))?
        .cast(&DataType::UInt64)
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?;
    let ranks = ranks
        .u64()
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?;
    let scores = df
        .column("score")
        .map_err(|_| MasstrustError::MissingColumn("score".into()))?
        .cast(&DataType::Float64)
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?;
    let scores = scores
        .f64()
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?;

    let probs = opt_f64!("probability");
    let smiles = opt_str!("smiles");
    let inchikeys = opt_str!("inchikey");
    let formulas = opt_str!("formula");
    let target_inchikeys = opt_str!("target_inchikey");
    let is_corrects = opt_str!("is_correct");

    let n = df.height();
    let mut candidates = Vec::with_capacity(n);
    for i in 0..n {
        let query_id = query_ids
            .get(i)
            .ok_or_else(|| MasstrustError::Parquet(format!("null query_id at row {i}")))?
            .to_string();
        let candidate_id = candidate_ids
            .get(i)
            .ok_or_else(|| MasstrustError::Parquet(format!("null candidate_id at row {i}")))?
            .to_string();
        let rank = ranks
            .get(i)
            .ok_or_else(|| MasstrustError::Parquet(format!("null rank at row {i}")))?
            as usize;
        let score = scores
            .get(i)
            .ok_or_else(|| MasstrustError::Parquet(format!("null score at row {i}")))?;

        let is_correct = is_corrects
            .as_ref()
            .and_then(|ca| ca.get(i))
            .and_then(|s| s.parse::<bool>().ok());

        candidates.push(Candidate {
            query_id,
            candidate_id,
            rank,
            score,
            probability: probs.as_ref().and_then(|ca| ca.get(i)),
            smiles: smiles.as_ref().and_then(|ca| ca.get(i)).map(str::to_string),
            inchikey: inchikeys
                .as_ref()
                .and_then(|ca| ca.get(i))
                .map(str::to_string),
            formula: formulas
                .as_ref()
                .and_then(|ca| ca.get(i))
                .map(str::to_string),
            target_inchikey: target_inchikeys
                .as_ref()
                .and_then(|ca| ca.get(i))
                .map(str::to_string),
            is_correct,
            // Not read from Parquet, same as the CSV path — populated afterward via
            // `read_group_column` if grouped calibration is requested.
            group: None,
        });
    }
    Ok(candidates)
}

/// Group a flat list of candidates into per-query rankings.
///
/// The returned slice is sorted alphabetically by `query_id` for deterministic output.
pub fn group_by_query(candidates: Vec<Candidate>) -> Vec<QueryRanking> {
    let mut map: HashMap<String, Vec<Candidate>> = HashMap::new();
    for c in candidates {
        map.entry(c.query_id.clone()).or_default().push(c);
    }
    let mut keys: Vec<String> = map.keys().cloned().collect();
    keys.sort();
    keys.into_iter()
        .map(|k| {
            let candidates = map.remove(&k).unwrap();
            QueryRanking {
                query_id: k,
                candidates,
            }
        })
        .collect()
}

/// Serialize `rows` to a CSV file at `path`, writing a header row derived from field names.
pub fn write_csv<T: serde::Serialize>(rows: &[T], path: &Path) -> Result<(), MasstrustError> {
    let mut wtr = csv::Writer::from_path(path)?;
    for row in rows {
        wtr.serialize(row)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn write_json<T: serde::Serialize>(value: &T, path: &Path) -> Result<(), MasstrustError> {
    let file = std::fs::File::create(path)?;
    serde_json::to_writer_pretty(file, value)?;
    Ok(())
}

/// Read a single named column from a CSV file, in row order.
///
/// Returns `None` for empty cells.  Returns [`MasstrustError::MissingColumn`] if
/// `col` is not in the header row.
pub fn read_group_column(path: &Path, col: &str) -> Result<Vec<Option<String>>, MasstrustError> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers = rdr.headers()?.clone();
    let idx = headers
        .iter()
        .position(|h| h == col)
        .ok_or_else(|| MasstrustError::MissingColumn(col.to_string()))?;
    rdr.records()
        .map(|r| Ok(r?.get(idx).filter(|s| !s.is_empty()).map(str::to_string)))
        .collect()
}

/// Read a named `[0, 1]` loss column (e.g. Tanimoto dissimilarity, scaffold mismatch) into a
/// `query_id -> value` map, for `certify-batch --loss-column` (see
/// `risksieve_backend::LossSource::Precomputed`).
///
/// Accepts **CSV** (any extension other than `.parquet`) or, with the `parquet` feature,
/// **Parquet** (auto-detected by extension) — matching [`read_candidates`]'s own convention.
/// Only rank-1 (top-1) rows contribute: the certified/realized loss is a property of the
/// top-ranked annotation per query, not of every candidate row. Reads `query_id`/`rank`/`col`
/// directly rather than reusing [`read_candidates`] plus a row-order zip, so a caller can never
/// end up pairing loss value `i` with the wrong query by an off-by-one row mismatch.
///
/// A cell that's empty/null is **not** an error here — that query is simply absent from the
/// returned map; whether that's an error depends on whether the caller actually needs a loss
/// for that query (a required calibration query vs. an optional, never-selected test query).
/// A cell that's present but not parseable as a number **is** always an error
/// ([`MasstrustError::InvalidLossValue`]), immediately, at read time — malformed data is never
/// silently treated as absent. A parseable value outside `[0, 1]` or non-finite is
/// [`MasstrustError::LossOutOfRange`], also at read time. Returns
/// [`MasstrustError::MissingColumn`] if `col` (or `query_id`/`rank`) is not in the header row —
/// callers use this to distinguish "no such column at all" (e.g. an unlabeled test set, not an
/// error) from "column exists but a value in it is bad" (always an error).
#[cfg(feature = "risksieve")]
pub fn read_query_loss_column(
    path: &Path,
    col: &str,
) -> Result<std::collections::BTreeMap<String, f64>, MasstrustError> {
    if path.extension().is_some_and(|e| e == "parquet") {
        #[cfg(feature = "parquet")]
        return read_query_loss_column_parquet(path, col);
        #[cfg(not(feature = "parquet"))]
        return Err(MasstrustError::ParquetNotEnabled);
    }
    read_query_loss_column_csv(path, col)
}

#[cfg(feature = "risksieve")]
fn read_query_loss_column_csv(
    path: &Path,
    col: &str,
) -> Result<std::collections::BTreeMap<String, f64>, MasstrustError> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers = rdr.headers()?.clone();
    let query_idx = headers
        .iter()
        .position(|h| h == "query_id")
        .ok_or_else(|| MasstrustError::MissingColumn("query_id".to_string()))?;
    let rank_idx = headers
        .iter()
        .position(|h| h == "rank")
        .ok_or_else(|| MasstrustError::MissingColumn("rank".to_string()))?;
    let col_idx = headers
        .iter()
        .position(|h| h == col)
        .ok_or_else(|| MasstrustError::MissingColumn(col.to_string()))?;

    let mut map = std::collections::BTreeMap::new();
    for record in rdr.records() {
        let record = record?;
        // `read_candidates` (via serde) is the authority on a malformed `rank`/`query_id` in
        // this same file; this auxiliary reader only needs to identify rank-1 rows, so a
        // rank that doesn't parse is simply not rank 1, not a second place to raise a
        // duplicate rank/query_id error.
        let rank: usize = record
            .get(rank_idx)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if rank != 1 {
            continue;
        }
        let query_id = record.get(query_idx).unwrap_or("").to_string();
        let raw = record.get(col_idx).unwrap_or("");
        if raw.is_empty() {
            continue;
        }
        let value: f64 = raw.parse().map_err(|_| MasstrustError::InvalidLossValue {
            query_id: query_id.clone(),
            column: col.to_string(),
            raw: raw.to_string(),
        })?;
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(MasstrustError::LossOutOfRange {
                query_id,
                column: col.to_string(),
                value,
            });
        }
        map.insert(query_id, value);
    }
    Ok(map)
}

#[cfg(all(feature = "risksieve", feature = "parquet"))]
fn read_query_loss_column_parquet(
    path: &Path,
    // Named `column_name`, not `col` -- `polars::prelude::*` below brings in its own `col()`
    // expression-builder function, which would otherwise shadow a parameter named `col`.
    column_name: &str,
) -> Result<std::collections::BTreeMap<String, f64>, MasstrustError> {
    use polars::prelude::*;

    let df = LazyFrame::scan_parquet(path, ScanArgsParquet::default())
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?
        .collect()
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?;

    let query_ids = df
        .column("query_id")
        .map_err(|_| MasstrustError::MissingColumn("query_id".to_string()))?
        .str()
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?
        .clone();
    let ranks = df
        .column("rank")
        .map_err(|_| MasstrustError::MissingColumn("rank".to_string()))?
        .cast(&DataType::UInt64)
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?;
    let ranks = ranks
        .u64()
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?;
    // A whole-column cast failure (e.g. the named column holds non-numeric text) surfaces here
    // as a single MasstrustError::Parquet -- the Parquet-side equivalent of a malformed CSV
    // cell, at column granularity rather than per-cell (polars has no cheaper way to isolate
    // which individual row failed a numeric cast).
    let loss_col = df
        .column(column_name)
        .map_err(|_| MasstrustError::MissingColumn(column_name.to_string()))?
        .cast(&DataType::Float64)
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?;
    let loss_col = loss_col
        .f64()
        .map_err(|e| MasstrustError::Parquet(e.to_string()))?;

    let mut map = std::collections::BTreeMap::new();
    for i in 0..df.height() {
        if ranks.get(i) != Some(1) {
            continue;
        }
        let Some(query_id) = query_ids.get(i) else {
            continue;
        };
        let Some(value) = loss_col.get(i) else {
            continue; // null -- missing, not an error (see doc comment on the public fn)
        };
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(MasstrustError::LossOutOfRange {
                query_id: query_id.to_string(),
                column: column_name.to_string(),
                value,
            });
        }
        map.insert(query_id.to_string(), value);
    }
    Ok(map)
}

pub fn read_policy(path: &Path) -> Result<PolicyFile, MasstrustError> {
    let file = std::fs::File::open(path)?;
    let policy: PolicyFile = serde_json::from_reader(file)?;
    if policy.version != POLICY_VERSION {
        return Err(MasstrustError::UnknownVersion(policy.version));
    }
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_csv(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn test_read_candidates_valid() {
        let f = write_temp_csv(
            "query_id,candidate_id,rank,score,probability,is_correct\n\
             q1,c1,1,0.9,0.7,true\n\
             q1,c2,2,0.8,0.3,false\n",
        );
        let candidates = read_candidates(f.path()).unwrap();
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].query_id, "q1");
        assert_eq!(candidates[0].rank, 1);
        assert_eq!(candidates[0].probability, Some(0.7));
        assert_eq!(candidates[0].is_correct, Some(true));
    }

    #[test]
    fn test_read_candidates_missing_required_column() {
        let f = write_temp_csv("query_id,candidate_id,rank\nq1,c1,1\n");
        let err = read_candidates(f.path()).unwrap_err();
        assert!(matches!(err, MasstrustError::MissingColumn(ref c) if c == "score"));
    }

    #[test]
    fn test_read_candidates_optional_columns_absent() {
        let f = write_temp_csv("query_id,candidate_id,rank,score\nq1,c1,1,0.9\n");
        let candidates = read_candidates(f.path()).unwrap();
        assert_eq!(candidates[0].probability, None);
        assert_eq!(candidates[0].is_correct, None);
    }

    #[test]
    fn test_read_candidates_empty() {
        let f = write_temp_csv("query_id,candidate_id,rank,score\n");
        let candidates = read_candidates(f.path()).unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_group_by_query_sorted() {
        let candidates = vec![
            Candidate {
                query_id: "qb".into(),
                candidate_id: "c1".into(),
                rank: 1,
                score: 0.9,
                probability: None,
                smiles: None,
                inchikey: None,
                target_inchikey: None,
                formula: None,
                is_correct: None,
                group: None,
            },
            Candidate {
                query_id: "qa".into(),
                candidate_id: "c2".into(),
                rank: 1,
                score: 0.8,
                probability: None,
                smiles: None,
                inchikey: None,
                target_inchikey: None,
                formula: None,
                is_correct: None,
                group: None,
            },
        ];
        let groups = group_by_query(candidates);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].query_id, "qa");
        assert_eq!(groups[1].query_id, "qb");
    }

    #[cfg(feature = "risksieve")]
    #[test]
    fn test_read_query_loss_column_only_rank_one_rows_contribute() {
        let f = write_temp_csv(
            "query_id,candidate_id,rank,score,tanimoto_loss\n\
             q1,c1,1,0.9,0.25\n\
             q1,c2,2,0.8,0.99\n\
             q2,c1,1,0.7,0.0\n",
        );
        let map = read_query_loss_column(f.path(), "tanimoto_loss").unwrap();
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("q1"), Some(&0.25));
        assert_eq!(map.get("q2"), Some(&0.0));
    }

    #[cfg(feature = "risksieve")]
    #[test]
    fn test_read_query_loss_column_empty_cell_is_absent_not_an_error() {
        let f = write_temp_csv("query_id,candidate_id,rank,score,tanimoto_loss\nq1,c1,1,0.9,\n");
        let map = read_query_loss_column(f.path(), "tanimoto_loss").unwrap();
        assert!(map.is_empty());
    }

    #[cfg(feature = "risksieve")]
    #[test]
    fn test_read_query_loss_column_malformed_value_is_a_hard_error() {
        let f = write_temp_csv(
            "query_id,candidate_id,rank,score,tanimoto_loss\nq1,c1,1,0.9,not_a_number\n",
        );
        let err = read_query_loss_column(f.path(), "tanimoto_loss").unwrap_err();
        assert!(matches!(
            err,
            MasstrustError::InvalidLossValue { ref query_id, ref raw, .. }
                if query_id == "q1" && raw == "not_a_number"
        ));
    }

    #[cfg(feature = "risksieve")]
    #[test]
    fn test_read_query_loss_column_out_of_range_value_is_a_hard_error() {
        let f = write_temp_csv("query_id,candidate_id,rank,score,tanimoto_loss\nq1,c1,1,0.9,1.5\n");
        let err = read_query_loss_column(f.path(), "tanimoto_loss").unwrap_err();
        assert!(matches!(
            err,
            MasstrustError::LossOutOfRange { ref query_id, value, .. }
                if query_id == "q1" && value == 1.5
        ));
    }

    #[cfg(feature = "risksieve")]
    #[test]
    fn test_read_query_loss_column_missing_column_header_is_an_error() {
        let f = write_temp_csv("query_id,candidate_id,rank,score\nq1,c1,1,0.9\n");
        let err = read_query_loss_column(f.path(), "tanimoto_loss").unwrap_err();
        assert!(matches!(err, MasstrustError::MissingColumn(ref c) if c == "tanimoto_loss"));
    }

    #[cfg(all(feature = "risksieve", feature = "parquet"))]
    #[test]
    fn test_read_query_loss_column_parquet_happy_path_and_null() {
        use polars::prelude::*;

        let mut df = df![
            "query_id" => ["q1", "q1", "q2"],
            "candidate_id" => ["c1", "c2", "c1"],
            "rank" => [1u64, 2, 1],
            "score" => [0.9f64, 0.8, 0.7],
            "tanimoto_loss" => [Some(0.25f64), Some(0.99), None],
        ]
        .unwrap();

        let f = NamedTempFile::new().unwrap();
        let file = std::fs::File::create(f.path()).unwrap();
        ParquetWriter::new(file).finish(&mut df).unwrap();

        let parquet_path = f.path().with_extension("parquet");
        std::fs::copy(f.path(), &parquet_path).unwrap();

        let map = read_query_loss_column(&parquet_path, "tanimoto_loss").unwrap();
        // rank-1 rows only: q1 (0.25) and q2 (null -- absent, not an error).
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("q1"), Some(&0.25));
        assert_eq!(map.get("q2"), None);

        std::fs::remove_file(&parquet_path).ok();
    }

    #[cfg(all(feature = "risksieve", feature = "parquet"))]
    #[test]
    fn test_read_query_loss_column_parquet_out_of_range_is_a_hard_error() {
        use polars::prelude::*;

        let mut df = df![
            "query_id" => ["q1"],
            "candidate_id" => ["c1"],
            "rank" => [1u64],
            "score" => [0.9f64],
            "tanimoto_loss" => [1.5f64],
        ]
        .unwrap();

        let f = NamedTempFile::new().unwrap();
        let file = std::fs::File::create(f.path()).unwrap();
        ParquetWriter::new(file).finish(&mut df).unwrap();
        let parquet_path = f.path().with_extension("parquet");
        std::fs::copy(f.path(), &parquet_path).unwrap();

        let err = read_query_loss_column(&parquet_path, "tanimoto_loss").unwrap_err();
        assert!(matches!(
            err,
            MasstrustError::LossOutOfRange { ref query_id, value, .. }
                if query_id == "q1" && value == 1.5
        ));

        std::fs::remove_file(&parquet_path).ok();
    }
}
