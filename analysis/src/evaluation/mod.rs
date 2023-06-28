use tak::State;

pub use self::features::GatherFeatures;
pub use self::model::{AnnEvaluator, AnnModel};
pub use self::types::Evaluation;

#[cfg(feature = "tools")]
pub mod explanation;

mod features;
mod model;
mod types;

pub trait Evaluator<const N: usize> {
    fn evaluate(&self, state: &State<N>) -> Evaluation;
}
