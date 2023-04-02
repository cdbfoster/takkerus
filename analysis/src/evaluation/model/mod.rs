use std::ops::{Deref, DerefMut};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use ann::shallow::ShallowAnn;
use tak::{Resolution, State};

use super::features::GatherFeatures;
use super::types::{EvalType, Evaluation};
use super::Evaluator;

pub trait AnnEvaluator<const N: usize> {
    const INPUTS: usize;
    const HIDDEN: usize;
    const OUTPUTS: usize;
    type Evaluator: for<'a> Deserialize<'a> + Serialize + Evaluator<N>;
    type Model;

    fn static_evaluator() -> &'static Self::Evaluator;
}

pub struct AnnModel<const N: usize>;

pub const EVAL_SCALE: f32 = 50_000.0;

macro_rules! model_impl {
    (size: $size:expr, module: $module:ident, model: $file:expr) => {
        mod $module {
            use super::*;

            const INPUTS: usize = <State<$size> as GatherFeatures>::FEATURES;
            const HIDDEN: usize = 10;
            const OUTPUTS: usize = 1;

            type InnerModel = ShallowAnn<INPUTS, HIDDEN, OUTPUTS>;

            #[derive(Deserialize, Serialize)]
            pub struct Model(pub(crate) InnerModel);

            static MODEL: Lazy<Model> = Lazy::new(|| {
                let data = include_str!($file);
                Model(serde_json::from_str(&data).expect("could not parse model data"))
            });

            impl<const N: usize> AsRef<dyn Evaluator<N> + 'static> for Model {
                fn as_ref(&self) -> &(dyn Evaluator<N> + 'static) {
                    debug_assert_eq!($size, N);
                    unsafe { std::mem::transmute(self as &dyn Evaluator<$size>) }
                }
            }

            impl Evaluator<$size> for Model {
                fn evaluate(&self, state: &State<$size>) -> Evaluation {
                    match state.resolution() {
                        None => (),
                        Some(Resolution::Road(color)) | Some(Resolution::Flats { color, .. }) => {
                            if color == state.to_move() {
                                return Evaluation::WIN - state.ply_count as i32;
                            } else {
                                return Evaluation::LOSE + state.ply_count as i32;
                            }
                        }
                        Some(Resolution::Draw) => return Evaluation::ZERO - state.ply_count as i32,
                    }

                    let features = state.gather_features();
                    let results = self.propagate_forward(features.as_vector().into());

                    Evaluation((results[0][0] * EVAL_SCALE) as EvalType)
                }
            }

            impl Deref for Model {
                type Target = InnerModel;
                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl DerefMut for Model {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.0
                }
            }

            impl From<InnerModel> for Model {
                fn from(model: InnerModel) -> Model {
                    Model(model)
                }
            }

            impl AnnEvaluator<$size> for AnnModel<$size> {
                const INPUTS: usize = INPUTS;
                const HIDDEN: usize = HIDDEN;
                const OUTPUTS: usize = OUTPUTS;
                type Evaluator = Model;
                type Model = InnerModel;

                fn static_evaluator() -> &'static Self::Evaluator {
                    &*MODEL
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
