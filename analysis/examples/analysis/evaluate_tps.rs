use std::fs::File;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use ::analysis::evaluation::{AnnEvaluator, AnnModel, Evaluator, EVAL_SCALE};
use tak::{State, Tps};

#[pyfunction]
pub fn evaluate_tps(tps_string: String, model_file: Option<String>) -> PyResult<f32> {
    let tps: Tps = tps_string
        .parse()
        .map_err(|_| PyValueError::new_err("could not parse tps"))?;

    macro_rules! sized {
        ($size:expr) => {{
            let state: State<$size> = tps
                .try_into()
                .map_err(|_| PyValueError::new_err("could not create state from tps"))?;

            if let Some(model_file) = model_file {
                let file = File::open(model_file)?;

                let evaluator: <AnnModel<$size> as AnnEvaluator<$size>>::Evaluator =
                    serde_json::from_reader(file)
                        .map_err(|_| PyValueError::new_err("could not deserialize model"))?;

                evaluator.evaluate(&state)
            } else {
                AnnModel::<$size>::static_evaluator().evaluate(&state)
            }
        }};
    }

    let evaluation = match tps.size() {
        3 => sized!(3),
        4 => sized!(4),
        5 => sized!(5),
        6 => sized!(6),
        7 => sized!(7),
        8 => sized!(8),
        _ => unreachable!(),
    };

    Ok(evaluation.into_f32() / EVAL_SCALE)
}
