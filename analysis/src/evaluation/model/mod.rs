use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use ann::shallow::ShallowAnn;
use tak::State;

use super::features::GatherFeatures;
use super::types::{EvalType, Evaluation};

pub trait Evaluator {
    const INPUTS: usize;
    const HIDDEN: usize;
    const OUTPUTS: usize;
    type State;
    type Model: for<'a> Deserialize<'a> + Serialize;

    fn static_model() -> &'static Self::Model;
    fn evaluate_model(model: &Self::Model, state: &Self::State) -> Evaluation;
}

pub struct Model<const N: usize>;

const EVAL_SCALE: f32 = 2_000.0;

macro_rules! model_impl {
    (size: $size:expr, module: $module:ident, model: $file:expr) => {
        mod $module {
            use super::*;

            const INPUTS: usize = <State<$size> as GatherFeatures>::FEATURES;
            const HIDDEN: usize = 10;
            const OUTPUTS: usize = 1;
            type ModelType = ShallowAnn<INPUTS, HIDDEN, OUTPUTS>;

            static MODEL: Lazy<ModelType> = Lazy::new(|| {
                let data = include_str!($file);
                serde_json::from_str(&data).expect("could not parse model data")
            });

            impl Evaluator for Model<$size> {
                const INPUTS: usize = INPUTS;
                const HIDDEN: usize = HIDDEN;
                const OUTPUTS: usize = OUTPUTS;
                type State = State<$size>;
                type Model = ModelType;

                fn static_model() -> &'static Self::Model {
                    &*MODEL
                }

                fn evaluate_model(model: &Self::Model, state: &Self::State) -> Evaluation {
                    let features = state.gather_features();
                    let results = model.propagate_forward(features.as_vector().into());

                    Evaluation((results[0][0] * EVAL_SCALE) as EvalType)
                }
            }
        }
    };
}

model_impl!(
    size: 3,
    module: model_3s,
    model: "model_3s.json"
);

model_impl!(
    size: 4,
    module: model_4s,
    model: "model_4s.json"
);

model_impl!(
    size: 5,
    module: model_5s,
    model: "model_5s.json"
);

model_impl!(
    size: 6,
    module: model_6s,
    model: "model_6s.json"
);

model_impl!(
    size: 7,
    module: model_7s,
    model: "model_7s.json"
);

model_impl!(
    size: 8,
    module: model_8s,
    model: "model_8s.json"
);
