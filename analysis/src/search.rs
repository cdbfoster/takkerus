use std::ops::{Add, AddAssign};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tracing::{debug, error, info, instrument, warn};

use tak::{Ply, State};

use crate::evaluation::{evaluate, Evaluation};
use crate::ply_generator::{Fallibility, PlyGenerator};
use crate::transposition_table::{Bound, TranspositionTable, TranspositionTableEntry};

#[derive(Debug, Default)]
pub struct AnalysisConfig<'a, const N: usize> {
    pub depth_limit: Option<u32>,
    pub time_limit: Option<Duration>,
    /// If this is set and the next search depth is predicted to take
    /// longer than the time limit, stop the search early.
    pub predict_time: bool,
    pub interrupted: Arc<AtomicBool>,
    /// A place to put data gathered during the search that could be
    /// useful to future searches. If none, this will be created internally.
    pub persistent_state: Option<&'a mut PersistentState<N>>,
}

#[derive(Debug)]
pub struct PersistentState<const N: usize> {
    transposition_table: TranspositionTable<N>,
}

impl<const N: usize> Default for PersistentState<N> {
    fn default() -> Self {
        Self {
            transposition_table: TranspositionTable::with_capacity(10_000_000),
        }
    }
}

#[derive(Debug)]
pub struct Analysis<const N: usize> {
    pub depth: u32,
    pub final_state: State<N>,
    pub evaluation: Evaluation,
    pub principal_variation: Vec<Ply<N>>,
    pub stats: Statistics,
    pub time: Duration,
}

#[derive(Clone, Debug, Default)]
pub struct Statistics {
    pub visited: u64,
    pub evaluated: u64,
    pub terminal: u64,
    pub scouted: u64,
    pub re_searched: u64,
    pub beta_cutoff: u64,
    pub tt_stores: u64,
    pub tt_store_fails: u64,
    pub tt_hits: u64,
    pub tt_saves: u64,
}

impl Add for &Statistics {
    type Output = Statistics;

    fn add(self, other: Self) -> Self::Output {
        Self::Output {
            visited: self.visited + other.visited,
            evaluated: self.evaluated + other.evaluated,
            terminal: self.terminal + other.terminal,
            scouted: self.scouted + other.scouted,
            re_searched: self.re_searched + other.re_searched,
            beta_cutoff: self.beta_cutoff + other.beta_cutoff,
            tt_stores: self.tt_stores + other.tt_stores,
            tt_store_fails: self.tt_store_fails + other.tt_store_fails,
            tt_hits: self.tt_hits + other.tt_hits,
            tt_saves: self.tt_saves + other.tt_saves,
        }
    }
}

impl AddAssign<&Statistics> for Statistics {
    fn add_assign(&mut self, other: &Self) {
        *self = self.add(other)
    }
}

#[instrument(level = "trace")]
pub fn analyze<const N: usize>(config: AnalysisConfig<N>, state: &State<N>) -> Analysis<N> {
    info!(
        depth_limit = %if let Some(depth_limit) = config.depth_limit {
            depth_limit.to_string()
        } else {
            "None".to_owned()
        },
        time_limit = %if let Some(time_limit) = config.time_limit {
            format!("{:.2}s", time_limit.as_secs_f64())
        } else {
            "None".to_owned()
        },
        "Analyzing...",
    );

    let total_start_time = Instant::now();

    let cancel_timer = if config.time_limit.is_some() {
        let cancel = spawn_timing_interrupt_thread(&config, total_start_time);
        Some(cancel)
    } else {
        None
    };

    let max_depth = config.depth_limit.unwrap_or(u32::MAX) as usize;

    // Use the passed-in persistent state or create a local one for this analysis.
    let mut local_persistent_state;
    let persistent_state = if let Some(persistent_state) = config.persistent_state {
        persistent_state
    } else {
        local_persistent_state = PersistentState::default();
        &mut local_persistent_state
    };

    let mut principal_variation = Vec::with_capacity(max_depth.min(15));

    let mut analyzed_depth = 0;
    let mut state = state.clone();
    let mut evaluation = Evaluation::ZERO;

    let mut branching_factors = Vec::with_capacity(max_depth.min(15));
    let mut previous_stats = Statistics::default();
    let mut total_stats = Statistics::default();

    for depth in 1..=max_depth {
        let depth_start_time = Instant::now();

        let mut search = SearchState {
            search_depth: depth,
            stats: Default::default(),
            interrupted: &config.interrupted,
            persistent_state,
        };

        evaluation = minimax(
            &mut search,
            &mut state,
            &mut principal_variation,
            depth,
            Evaluation::MIN,
            Evaluation::MAX,
        );

        analyzed_depth = depth as u32;
        total_stats += &search.stats;

        fetch_pv(
            &state,
            &search.persistent_state.transposition_table,
            analyzed_depth as usize,
            &mut principal_variation,
        );

        let total_time = total_start_time.elapsed();
        let depth_time = depth_start_time.elapsed();

        if config.interrupted.load(Ordering::Relaxed) {
            break;
        }

        // Calculate previous and average branching factors.
        let branching_factor =
            search.stats.evaluated as f64 / previous_stats.evaluated.max(1) as f64;
        branching_factors.push(branching_factor);
        let average_branching_factor =
            branching_factors.iter().sum::<f64>() / branching_factors.len() as f64;

        info!(
            depth = analyzed_depth,
            eval = %format!("{:<4}", evaluation),
            pv = ?principal_variation,
            "Analyzed:",
        );

        debug!(
            visited = search.stats.visited,
            evaluated = search.stats.evaluated,
            terminal = search.stats.terminal,
            scouted = search.stats.scouted,
            re_searched = search.stats.re_searched,
            beta_cutoff = search.stats.beta_cutoff,
            tt_stores = search.stats.tt_stores,
            tt_store_fails = search.stats.tt_store_fails,
            tt_hits = search.stats.tt_hits,
            tt_saves = search.stats.tt_saves,
            "Stats:",
        );

        debug!(
            branch = %format!("{:.2}", branching_factor),
            avg_branch = %format!("{:.2}", average_branching_factor),
            rate = %format!("{} nodes/s", (search.stats.visited as f64 / depth_time.as_secs_f64()) as u64),
            "Search:",
        );

        if evaluation.is_terminal() {
            info!("Tinuë found. Stopping.");
            break;
        }

        if config.predict_time {
            if let Some(time_limit) = config.time_limit {
                let next_depth_prediction = depth_time * average_branching_factor.round() as u32;

                if total_time + next_depth_prediction > time_limit {
                    info!(
                        time = %format!("{:.2}s", total_time.as_secs_f64()),
                        limit = %format!("{:.2}s", time_limit.as_secs_f64()),
                        prediction = %format!("{:.2}s", next_depth_prediction.as_secs_f64()),
                        "Next depth is predicted to take too long. Stopping."
                    );
                    break;
                }
            }
        }

        previous_stats = search.stats.clone();
    }

    // If we started a timer, stop it.
    if let Some(cancel_timer) = cancel_timer {
        cancel_timer.store(true, Ordering::Relaxed);
    }

    Analysis {
        depth: analyzed_depth,
        final_state: state,
        evaluation,
        principal_variation,
        stats: total_stats,
        time: total_start_time.elapsed(),
    }
}

struct SearchState<'a, const N: usize> {
    search_depth: usize,
    stats: Statistics,
    interrupted: &'a AtomicBool,
    persistent_state: &'a mut PersistentState<N>,
}

fn fetch_pv<const N: usize>(
    state: &State<N>,
    tt: &TranspositionTable<N>,
    max_depth: usize,
    pv: &mut Vec<Ply<N>>,
) {
    if pv.len() >= max_depth {
        return;
    }

    let mut state = state.clone();

    for ply in pv.iter() {
        if let Err(err) = state.execute_ply(*ply) {
            error!(error = ?err, "Principal variation ply caused an error. Skipping");
        }
    }

    while let Some(entry) = tt.get(state.metadata.hash) {
        if let Err(err) = state.execute_ply(entry.ply) {
            error!(error = ?err, "Principal variation ply caused an error. Skipping");
        } else {
            pv.push(entry.ply);

            // Only grab as many as we've actually analyzed.
            if pv.len() == max_depth {
                break;
            }
        }
    }
}

#[instrument(level = "trace", skip(search), fields(scout = alpha + 1 == beta))]
fn minimax<const N: usize>(
    search: &mut SearchState<'_, N>,
    state: &mut State<N>,
    principal_variation: &mut Vec<Ply<N>>,
    depth: usize,
    mut alpha: Evaluation,
    beta: Evaluation,
) -> Evaluation {
    search.stats.visited += 1;

    if depth == 0 || state.resolution().is_some() {
        search.stats.evaluated += 1;
        principal_variation.clear();
        return evaluate(state);
    }

    if alpha + 1 == beta {
        search.stats.scouted += 1;
    }

    let mut tt_entry = search
        .persistent_state
        .transposition_table
        .get(state.metadata.hash);

    if let Some(entry) = tt_entry {
        search.stats.tt_hits += 1;

        let is_save = entry.depth as usize >= depth
            && match entry.bound {
                Bound::Exact => true,
                Bound::Upper => entry.evaluation <= alpha,
                Bound::Lower => entry.evaluation >= beta,
            };

        let is_terminal = entry.bound == Bound::Exact && entry.evaluation.is_terminal();

        if is_save || is_terminal {
            if state.validate_ply(entry.ply).is_ok() {
                search.stats.tt_saves += 1;

                principal_variation.clear();
                principal_variation.push(entry.ply);

                return entry.evaluation;
            } else {
                tt_entry = None;
            }
        }
    }

    let ply_generator = PlyGenerator::new(
        state,
        principal_variation.first().copied(),
        tt_entry.map(|e| e.ply),
    );

    let mut next_pv = if !principal_variation.is_empty() {
        principal_variation[1..].to_vec()
    } else {
        Vec::with_capacity(search.search_depth - depth)
    };

    let mut first_iteration = true;
    let mut raised_alpha = false;

    for (fallibility, ply) in ply_generator {
        let mut state = state.clone();

        use Fallibility::*;
        match fallibility {
            Fallible => {
                if state.execute_ply(ply).is_err() {
                    continue;
                }
            }
            Infallible => state.execute_ply_unchecked(ply),
        }

        let next_eval = if first_iteration {
            // On the first iteration, perform a full-window search.
            -minimax(search, &mut state, &mut next_pv, depth - 1, -beta, -alpha)
        } else {
            // Afterwards, perform a null-window search, expecting to fail low (counting
            // on our move ordering to have already led us to the "best" move).
            let scout_eval = -minimax(
                search,
                &mut state,
                &mut next_pv,
                depth - 1,
                -alpha - 1,
                -alpha,
            );

            if scout_eval > alpha && scout_eval < beta {
                search.stats.re_searched += 1;
                // If we fail high instead, we need to re-search using the full window.
                -minimax(search, &mut state, &mut next_pv, depth - 1, -beta, -alpha)
            } else {
                scout_eval
            }
        };

        if next_eval > alpha {
            alpha = next_eval;
            raised_alpha = true;

            principal_variation.clear();
            principal_variation.push(ply);
            principal_variation.extend_from_slice(&next_pv);

            if alpha >= beta {
                search.stats.beta_cutoff += 1;
                break;
            }
        } else if principal_variation.is_empty() {
            principal_variation.push(ply);
        }

        first_iteration = false;

        if search.interrupted.load(Ordering::Relaxed) {
            return alpha;
        }
    }

    if let Some(ply) = principal_variation.first().copied() {
        let inserted = search.persistent_state.transposition_table.insert(
            state.metadata.hash,
            TranspositionTableEntry {
                bound: if alpha >= beta {
                    Bound::Lower
                } else if raised_alpha {
                    Bound::Exact
                } else {
                    Bound::Upper
                },
                evaluation: alpha,
                depth: depth as u8,
                ply_count: (state.ply_count & 0xFF) as u8,
                ply,
            },
        );

        if inserted {
            search.stats.tt_stores += 1;
        } else {
            search.stats.tt_store_fails += 1;
        }
    }

    alpha
}

fn spawn_timing_interrupt_thread<const N: usize>(
    config: &AnalysisConfig<N>,
    start_time: Instant,
) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));

    {
        let cancel = cancel.clone();
        let time_limit = config.time_limit.unwrap();
        let interrupted = config.interrupted.clone();
        thread::spawn(move || loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }

            let remaining_time = time_limit.saturating_sub(start_time.elapsed());
            if !remaining_time.is_zero() {
                // Check for cancels at least every 10th of a second.
                let sleep_time = remaining_time.div_f64(2.0).min(Duration::from_millis(100));
                thread::sleep(sleep_time);
            } else {
                info!("Time limit reached. Stopping.");
                interrupted.store(true, Ordering::Relaxed);
                break;
            }
        });
    }

    cancel
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing_subscriber::filter::EnvFilter;
    use tracing_subscriber::fmt::format;
    use tracing_subscriber::fmt::time::Uptime;

    fn setup() {
        let event_format = format().with_timer(Uptime::default());

        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .event_format(event_format)
            .try_init()
            .ok();
    }

    #[test]
    #[ignore]
    fn depth_limit_opening() {
        setup();

        let config = AnalysisConfig {
            depth_limit: Some(7),
            ..Default::default()
        };
        let state = State::<6>::default();
        let analysis = analyze(config, &state);

        assert_eq!(analysis.depth, 7);
    }

    #[test]
    #[ignore]
    fn time_limit_opening() {
        setup();

        let time_limit = Duration::from_secs(60);
        let start_time = Instant::now();

        let config = AnalysisConfig {
            time_limit: Some(time_limit),
            ..Default::default()
        };
        let state = State::<6>::default();
        let analysis = analyze(config, &state);

        let elapsed = start_time.elapsed();

        debug!(?analysis, "Final analysis:");

        // Give it a bit of grace.
        assert!(elapsed < time_limit + Duration::from_millis(10));
    }

    #[test]
    #[ignore]
    fn time_limit_with_predict_opening() {
        setup();

        let time_limit = Duration::from_secs(5);
        let start_time = Instant::now();

        let config = AnalysisConfig {
            time_limit: Some(time_limit),
            predict_time: true,
            ..Default::default()
        };
        let state = State::<6>::default();
        let _analysis = analyze(config, &state);

        let elapsed = start_time.elapsed();

        // Should be comfortably under time.
        assert!(elapsed < time_limit);
    }
}
