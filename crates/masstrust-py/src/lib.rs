use std::path::Path;

use masstrust_core::{
    calibration::{calibrate_binomial, calibrate_empirical},
    io, metrics, policy, CalibrationMethod, PolicyFile, ScoringMethod,
};
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

fn map_err(e: impl std::fmt::Display) -> PyErr {
    PyRuntimeError::new_err(e.to_string())
}

fn parse_score(s: &str) -> PyResult<ScoringMethod> {
    match s {
        "max-prob" | "max_prob" => Ok(ScoringMethod::MaxProb),
        "score-gap" | "score_gap" => Ok(ScoringMethod::ScoreGap),
        "margin" => Ok(ScoringMethod::Margin),
        "entropy" => Ok(ScoringMethod::Entropy),
        o => Err(PyValueError::new_err(format!(
            "Unknown score method '{o}'. Valid: max-prob, score-gap, margin, entropy"
        ))),
    }
}

fn parse_cal(s: &str) -> PyResult<CalibrationMethod> {
    match s {
        "empirical" => Ok(CalibrationMethod::Empirical),
        "binomial" => Ok(CalibrationMethod::Binomial),
        o => Err(PyValueError::new_err(format!(
            "Unknown calibration method '{o}'. Valid: empirical, binomial"
        ))),
    }
}

fn policy_to_dict<'py>(py: Python<'py>, pf: &PolicyFile) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new_bound(py);
    d.set_item("version", &pf.version)?;
    let scoring_json = serde_json::to_value(&pf.scoring_method).map_err(map_err)?;
    d.set_item("scoring_method", scoring_json.as_str().unwrap_or(""))?;
    d.set_item("threshold", pf.threshold)?;
    d.set_item("target_error_rate", pf.target_error_rate)?;
    let cal_json = serde_json::to_value(&pf.calibration_method).map_err(map_err)?;
    d.set_item("calibration_method", cal_json.as_str().unwrap_or(""))?;
    d.set_item("confidence_level", pf.confidence_level)?;
    d.set_item("created_by", &pf.created_by)?;
    Ok(d)
}

fn dict_to_policy(d: &Bound<'_, PyDict>) -> PyResult<PolicyFile> {
    macro_rules! get {
        ($key:expr, $ty:ty) => {
            d.get_item($key)?
                .ok_or_else(|| PyKeyError::new_err(format!("missing '{}'", $key)))?
                .extract::<$ty>()?
        };
    }

    let scoring_str = get!("scoring_method", String);
    let scoring_method: ScoringMethod =
        serde_json::from_value(serde_json::Value::String(scoring_str)).map_err(map_err)?;

    let cal_str = get!("calibration_method", String);
    let calibration_method: CalibrationMethod =
        serde_json::from_value(serde_json::Value::String(cal_str)).map_err(map_err)?;

    let confidence_level: Option<f64> = d
        .get_item("confidence_level")?
        .filter(|v| !v.is_none())
        .map(|v| v.extract::<f64>())
        .transpose()?;

    Ok(PolicyFile {
        version: get!("version", String),
        scoring_method,
        threshold: get!("threshold", f64),
        target_error_rate: get!("target_error_rate", f64),
        calibration_method,
        confidence_level,
        created_by: "masstrust".into(),
    })
}

/// Compute the risk-coverage curve from a labeled candidates CSV.
/// Returns a list of dicts: threshold, accepted, total, coverage, errors, risk.
#[pyfunction]
#[pyo3(signature = (csv_path, score))]
fn compute_curve(py: Python<'_>, csv_path: &str, score: &str) -> PyResult<Py<PyList>> {
    let method = parse_score(score)?;
    let candidates = io::read_candidates(Path::new(csv_path)).map_err(map_err)?;
    let rankings = io::group_by_query(candidates);
    let curve = metrics::compute_curve(&rankings, method);

    let list = PyList::empty_bound(py);
    for row in &curve {
        let d = PyDict::new_bound(py);
        d.set_item("threshold", row.threshold)?;
        d.set_item("accepted", row.accepted)?;
        d.set_item("total", row.total)?;
        d.set_item("coverage", row.coverage)?;
        d.set_item("errors", row.errors)?;
        d.set_item("risk", row.risk)?;
        list.append(d)?;
    }
    Ok(list.unbind())
}

/// Compute AURC for a risk-coverage curve (list of dicts from compute_curve).
#[pyfunction]
fn aurc(py: Python<'_>, curve: Bound<'_, PyList>) -> PyResult<f64> {
    let rows = py_list_to_curve(py, &curve)?;
    Ok(metrics::compute_aurc(&rows))
}

/// Compute E-AURC for a risk-coverage curve.
#[pyfunction]
fn eaurc(py: Python<'_>, curve: Bound<'_, PyList>) -> PyResult<f64> {
    let rows = py_list_to_curve(py, &curve)?;
    Ok(metrics::compute_eaurc(&rows))
}

fn py_list_to_curve(
    _py: Python<'_>,
    list: &Bound<'_, PyList>,
) -> PyResult<Vec<masstrust_core::RiskCoverageRow>> {
    list.iter()
        .map(|item| {
            let d = item.downcast::<PyDict>()?;
            Ok(masstrust_core::RiskCoverageRow {
                threshold: d
                    .get_item("threshold")?
                    .ok_or_else(|| PyKeyError::new_err("missing 'threshold'"))?
                    .extract()?,
                accepted: d
                    .get_item("accepted")?
                    .ok_or_else(|| PyKeyError::new_err("missing 'accepted'"))?
                    .extract()?,
                total: d
                    .get_item("total")?
                    .ok_or_else(|| PyKeyError::new_err("missing 'total'"))?
                    .extract()?,
                coverage: d
                    .get_item("coverage")?
                    .ok_or_else(|| PyKeyError::new_err("missing 'coverage'"))?
                    .extract()?,
                errors: d
                    .get_item("errors")?
                    .ok_or_else(|| PyKeyError::new_err("missing 'errors'"))?
                    .extract()?,
                risk: d
                    .get_item("risk")?
                    .ok_or_else(|| PyKeyError::new_err("missing 'risk'"))?
                    .extract()?,
            })
        })
        .collect()
}

/// Calibrate a trust threshold and return a policy dict.
#[pyfunction]
#[pyo3(signature = (csv_path, score, error_rate, method="empirical", confidence_level=None))]
fn calibrate(
    py: Python<'_>,
    csv_path: &str,
    score: &str,
    error_rate: f64,
    method: &str,
    confidence_level: Option<f64>,
) -> PyResult<Py<PyDict>> {
    let scoring_method = parse_score(score)?;
    let cal_method = parse_cal(method)?;

    let candidates = io::read_candidates(Path::new(csv_path)).map_err(map_err)?;
    let rankings = io::group_by_query(candidates);
    let curve = metrics::compute_curve(&rankings, scoring_method);

    let threshold_opt = match cal_method {
        CalibrationMethod::Empirical => calibrate_empirical(&curve, error_rate),
        CalibrationMethod::Binomial => {
            let level = confidence_level.ok_or_else(|| {
                PyValueError::new_err("confidence_level required for binomial method")
            })?;
            calibrate_binomial(&curve, error_rate, level).map_err(map_err)?
        }
    };

    let threshold = threshold_opt.unwrap_or(f64::MAX);

    let pf = PolicyFile {
        version: "0.1.0".into(),
        scoring_method,
        threshold,
        target_error_rate: error_rate,
        calibration_method: cal_method,
        confidence_level,
        created_by: "masstrust".into(),
    };

    Ok(policy_to_dict(py, &pf)?.unbind())
}

/// Apply a policy dict to a candidates CSV. Returns a list of decision dicts.
#[pyfunction]
fn apply_policy(
    py: Python<'_>,
    csv_path: &str,
    policy_dict: Bound<'_, PyDict>,
) -> PyResult<Py<PyList>> {
    let pf = dict_to_policy(&policy_dict)?;
    let candidates = io::read_candidates(Path::new(csv_path)).map_err(map_err)?;
    let rankings = io::group_by_query(candidates);
    let decisions = policy::apply_policy(&rankings, &pf);

    let list = PyList::empty_bound(py);
    for d in &decisions {
        let dict = PyDict::new_bound(py);
        dict.set_item("query_id", &d.query_id)?;
        dict.set_item("candidate_id", &d.candidate_id)?;
        dict.set_item("confidence", d.confidence)?;
        dict.set_item("accepted", d.accepted)?;
        dict.set_item("threshold", d.threshold)?;
        dict.set_item("method", &d.method)?;
        list.append(dict)?;
    }
    Ok(list.unbind())
}

/// Load a policy JSON file. Returns a policy dict.
#[pyfunction]
fn load_policy(py: Python<'_>, path: &str) -> PyResult<Py<PyDict>> {
    let pf = policy::load_policy(Path::new(path)).map_err(map_err)?;
    Ok(policy_to_dict(py, &pf)?.unbind())
}

/// Save a policy dict to a JSON file.
#[pyfunction]
fn save_policy(path: &str, policy_dict: Bound<'_, PyDict>) -> PyResult<()> {
    let pf = dict_to_policy(&policy_dict)?;
    policy::save_policy(&pf, Path::new(path)).map_err(map_err)
}

#[pymodule]
fn masstrust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(compute_curve, m)?)?;
    m.add_function(wrap_pyfunction!(aurc, m)?)?;
    m.add_function(wrap_pyfunction!(eaurc, m)?)?;
    m.add_function(wrap_pyfunction!(calibrate, m)?)?;
    m.add_function(wrap_pyfunction!(apply_policy, m)?)?;
    m.add_function(wrap_pyfunction!(load_policy, m)?)?;
    m.add_function(wrap_pyfunction!(save_policy, m)?)?;
    Ok(())
}
