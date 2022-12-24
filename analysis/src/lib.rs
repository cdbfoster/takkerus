pub use evaluation::{evaluate, Evaluation};
pub use search::{analyze, Analysis, AnalysisConfig, PersistentState, Statistics};

mod evaluation;
mod ply_generator;
mod rng;
mod search;
