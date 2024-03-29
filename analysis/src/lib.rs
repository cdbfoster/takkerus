pub use self::analysis::{analyze, Analysis, AnalysisConfig, PersistentState};
pub use self::statistics::Statistics;
pub use self::time::TimeControl;
pub use self::transposition_table::{TranspositionTable, TranspositionTableEntry};
pub use self::util::Sender;

mod analysis;
pub mod evaluation;
mod move_order;
mod ply_generator;
mod search;
mod statistics;
mod time;
mod transposition_table;
mod util;

pub fn version() -> &'static str {
    option_env!("CARGO_PKG_VERSION").unwrap_or("(unknown version)")
}
