pub use self::search::{analyze, Analysis, AnalysisConfig, PersistentState};
pub use self::statistics::Statistics;
pub use self::transposition_table::{TranspositionTable, TranspositionTableEntry};
pub use self::util::Sender;

pub mod evaluation;
mod move_order;
mod plies;
mod search;
mod statistics;
mod transposition_table;
mod util;

pub fn version() -> &'static str {
    option_env!("CARGO_PKG_VERSION").unwrap_or("(unknown version)")
}
