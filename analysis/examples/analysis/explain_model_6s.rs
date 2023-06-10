use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::fs::File;

use ::analysis::evaluation::explanation::model::Model6sExplainer;
use ::analysis::evaluation::{AnnEvaluator, AnnModel, GatherFeatures};
use lime::{Explainer, Explanation as LimeExplanation};
use tak::State;

#[derive(Clone)]
#[pyclass]
pub struct Explanation {
    #[pyo3(get)]
    pub evaluation: f32,
    #[pyo3(get)]
    pub feature_weights: Vec<FeatureWeight>,
    #[pyo3(get)]
    pub intercept: f32,
}

#[derive(Clone)]
#[pyclass]
pub struct FeatureWeight {
    #[pyo3(get)]
    pub feature: String,
    #[pyo3(get)]
    pub weight: f32,
}

#[pyfunction]
pub fn explain_model_6s(
    tps_string: String,
    samples: usize,
    model_file: Option<String>,
) -> PyResult<Explanation> {
    let state: State<6> = tps_string
        .parse()
        .map_err(|_| PyValueError::new_err("could not parse tps"))?;

    let input = state.gather_features();

    let model = if let Some(model_file) = model_file {
        let file = File::open(model_file)?;

        serde_json::from_reader(file)
            .map_err(|_| PyValueError::new_err("could not deserialize model"))?
    } else {
        (*AnnModel::<6>::static_evaluator()).clone()
    };

    let explainer = Model6sExplainer { model };
    let explanation = explainer.explain(&input, samples);

    let LimeExplanation {
        evaluation,
        feature_weights,
        intercept,
    } = explanation;

    let feature_weights = feature_weights
        .into_iter()
        .map(|fw| FeatureWeight {
            feature: fw.feature,
            weight: fw.weight,
        })
        .collect();

    Ok(Explanation {
        evaluation,
        feature_weights,
        intercept,
    })
}
