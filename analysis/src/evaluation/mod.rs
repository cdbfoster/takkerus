use tak::State;

pub use self::features::GatherFeatures;
pub use self::model::{AnnEvaluator, AnnModel, EVAL_SCALE};
pub use self::types::Evaluation;

mod features;
mod model;
mod types;

pub trait Evaluator<const N: usize> {
    fn evaluate(&self, state: &State<N>) -> Evaluation;
}
