pub use self::evaluation::{evaluate, Evaluation};
pub use self::search::{analyze, Analysis, AnalysisConfig, PersistentState, Statistics};
pub use self::transposition_table::{TranspositionTable, TranspositionTableEntry};

mod evaluation;
mod ply_generator;
mod rng;
mod search;
mod transposition_table;
mod util;

pub fn version() -> &'static str {
    option_env!("CARGO_PKG_VERSION").unwrap_or("(unknown version)")
}
