pub use evaluation::{evaluate, Evaluation};
pub use search::{analyze, Analysis, AnalysisConfig, PersistentState, Statistics};

mod evaluation;
mod ply_generator;
mod rng;
mod search;

pub fn version() -> &'static str {
    option_env!("CARGO_PKG_VERSION").unwrap_or("(unknown version)")
}
