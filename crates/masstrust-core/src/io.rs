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
            is_correct,
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
}
