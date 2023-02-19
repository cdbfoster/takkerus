use once_cell::sync::Lazy;

use ann::shallow::ShallowAnn;
use tak::State;

use super::features::GatherFeatures;
use super::types::{EvalType, Evaluation};

pub trait Model: GatherFeatures {
    type Model;

    fn static_model() -> &'static Self::Model;
    fn run_inference(&self, model: &Self::Model) -> Evaluation;
}

pub fn evaluate_model<const N: usize>(
    state: &State<N>,
    model: &<State<N> as Model>::Model,
) -> Evaluation
where
    State<N>: Model,
{
    state.run_inference(model)
}

const EVAL_SCALE: f32 = 2_000.0;

macro_rules! model_impl {
    (size: $size:expr, module: $module:ident, model: $file:expr) => {
        mod $module {
            use super::*;

            static MODEL: Lazy<ShallowAnn<{ <State<$size> as GatherFeatures>::FEATURES }, 10, 1>> =
                Lazy::new(|| {
                    let data = include_str!($file);
                    serde_json::from_str(&data).expect("could not parse model data")
                });

            impl Model for State<$size> {
                type Model = ShallowAnn<{ <State<$size> as GatherFeatures>::FEATURES }, 10, 1>;

                fn static_model() -> &'static Self::Model {
                    &*MODEL
                }

                fn run_inference(&self, model: &Self::Model) -> Evaluation {
                    let features = self.gather_features();
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
