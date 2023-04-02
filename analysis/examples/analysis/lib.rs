use pyo3::prelude::*;

mod evaluate_tps;
mod explain_model_6s;

#[pymodule]
fn analysis(_py: Python<'_>, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(evaluate_tps::evaluate_tps, m)?)?;
    m.add_function(wrap_pyfunction!(explain_model_6s::explain_model_6s, m)?)?;
    Ok(())
}
