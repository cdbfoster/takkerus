use std::fmt::Write;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tracing::{debug, error, info, instrument, trace, trace_span, warn};

use tak::{Ply, State};

use crate::evaluation::{AnnEvaluator, AnnModel, Evaluation, Evaluator};
use crate::plies::{Fallibility, KillerMoves, PlyGenerator};
use crate::transposition_table::{Bound, TranspositionTable, TranspositionTableEntry};
use crate::util::{Neighbors, Sender};

#[derive(Default)]
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
    /// If false, the search is allowed to use unprovable methods that may
    /// improve playing strength at the cost of correctness.
    pub exact_eval: bool,
    /// The evaluator to use for the search. If none, a default will be
    /// used internally.
    pub evaluator: Option<&'a dyn Evaluator<N>>,
    /// A sender that will be used to send interim results during the search.
    pub interim_analysis_sender: Option<Box<dyn Sender<Analysis<N>>>>,
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

#[derive(Clone, Debug)]
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
    pub null_cutoff: u64,
    pub tt_stores: u64,
    pub tt_store_fails: u64,
    pub tt_hits: u64,
    pub tt_saves: u64,
    pub best_ply_order: [u64; 6],
}

impl Add for &Statistics {
    type Output = Statistics;

    fn add(self, other: Self) -> Self::Output {
        let mut best_ply_order = self.best_ply_order;
        for (a, b) in best_ply_order.iter_mut().zip(other.best_ply_order) {
            *a += b;
        }

        Self::Output {
            visited: self.visited + other.visited,
            evaluated: self.evaluated + other.evaluated,
            terminal: self.terminal + other.terminal,
            scouted: self.scouted + other.scouted,
            re_searched: self.re_searched + other.re_searched,
            beta_cutoff: self.beta_cutoff + other.beta_cutoff,
            null_cutoff: self.null_cutoff + other.null_cutoff,
            tt_stores: self.tt_stores + other.tt_stores,
            tt_store_fails: self.tt_store_fails + other.tt_store_fails,
            tt_hits: self.tt_hits + other.tt_hits,
            tt_saves: self.tt_saves + other.tt_saves,
            best_ply_order,
        }
    }
}

impl AddAssign<&Statistics> for Statistics {
    fn add_assign(&mut self, other: &Self) {
        *self = self.add(other)
    }
}

impl<'a> Sum<&'a Self> for Statistics {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        let mut total = Self::default();
        for x in iter {
            total += x;
        }
        total
    }
}

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

    let evaluator: &dyn Evaluator<N> = config.evaluator.unwrap_or_else(|| match N {
        3 => AnnModel::<3>::static_evaluator().as_ref(),
        4 => AnnModel::<4>::static_evaluator().as_ref(),
        5 => AnnModel::<5>::static_evaluator().as_ref(),
        6 => AnnModel::<6>::static_evaluator().as_ref(),
        7 => AnnModel::<7>::static_evaluator().as_ref(),
        8 => AnnModel::<8>::static_evaluator().as_ref(),
        _ => unreachable!(),
    });

    let mut analysis = Analysis {
        depth: 0,
        final_state: state.clone(),
        evaluation: evaluator.evaluate(state, state.resolution()),
        principal_variation: Vec::new(),
        stats: Statistics::default(),
        time: Duration::ZERO,
    };

    let search_start_time = Instant::now();

    for depth in 1..=max_depth {
        let depth_start_time = Instant::now();

        let mut search = SearchState {
            start_ply: state.ply_count,
            stats: Default::default(),
            interrupted: &config.interrupted,
            persistent_state,
            killer_moves: vec![KillerMoves::default(); depth],
            exact_eval: config.exact_eval,
            evaluator,
        };

        debug!(depth, "Beginning analysis...");

        let root = minimax(
            &mut search,
            state,
            depth,
            Evaluation::MIN,
            Evaluation::MAX,
            true,
        );

        if config.interrupted.load(Ordering::Relaxed) {
            break;
        }

        let (principal_variation, final_state) = fetch_pv(
            state,
            &search.persistent_state.transposition_table,
            root.depth,
        );

        let total_stats = search.stats.total();

        analysis = Analysis {
            depth: root.depth as u32,
            final_state,
            evaluation: root.evaluation,
            principal_variation,
            stats: &analysis.stats + &total_stats,
            time: search_start_time.elapsed(),
        };

        if let Some(sender) = &config.interim_analysis_sender {
            if let Err(error) = sender.send(analysis.clone()) {
                error!(?error, "Could not send interim analysis.");
            }
        }

        let depth_time = depth_start_time.elapsed();

        info!(
            depth = analysis.depth,
            time = %format!("{:05.2}s", analysis.time.as_secs_f64()),
            eval = %format!("{:<4}", analysis.evaluation),
            pv = ?analysis.principal_variation,
            "Analyzed:",
        );

        debug!(
            visited = total_stats.visited,
            evaluated = total_stats.evaluated,
            terminal = total_stats.terminal,
            scouted = total_stats.scouted,
            re_searched = total_stats.re_searched,
            beta_cutoff = total_stats.beta_cutoff,
            null_cutoff = total_stats.null_cutoff,
            tt_stores = total_stats.tt_stores,
            tt_store_fails = total_stats.tt_store_fails,
            tt_hits = total_stats.tt_hits,
            tt_saves = total_stats.tt_saves,
            tt_full = %format!(
                "{:05.2}%",
                100.0 * search.persistent_state.transposition_table.len() as f64
                    / search.persistent_state.transposition_table.capacity() as f64
            ),
            "Stats:",
        );

        // Best ply ordering calc.
        {
            let total = total_stats.best_ply_order.iter().sum::<u64>().max(1);
            let mut buffer = String::new();
            for (i, c) in total_stats.best_ply_order.into_iter().enumerate() {
                if i < 5 {
                    write!(
                        buffer,
                        "{}: {:5.2}%, ",
                        i + 1,
                        100.0 * c as f64 / total as f64
                    )
                    .ok();
                } else {
                    write!(buffer, "6+: {:5.2}%", 100.0 * c as f64 / total as f64).ok();
                }
            }

            debug!("Best ply ordering: {buffer}");
        }

        let branching_factor = effective_branching_factor(search.stats.iter().map(|d| d.visited));
        let next_depth_prediction = depth_time.as_secs_f64() * branching_factor;

        debug!(
            branch = %format!("{:.2}", branching_factor),
            rate = %format!("{}n/s", (total_stats.visited as f64 / depth_time.as_secs_f64()) as u64),
            next_depth_prediction = %format!("{:.2}s", analysis.time.as_secs_f64() + next_depth_prediction),
            "Search:",
        );

        if analysis.evaluation.is_terminal() {
            info!("Tinuë found. Stopping.");
            break;
        }

        if config.predict_time {
            if let Some(time_limit) = config.time_limit {
                if analysis.time + Duration::from_secs_f64(next_depth_prediction) > time_limit {
                    info!(
                        time = %format!("{:.2}s", analysis.time.as_secs_f64()),
                        limit = %format!("{:.2}s", time_limit.as_secs_f64()),
                        prediction = %format!("{:.2}s", analysis.time.as_secs_f64() + next_depth_prediction),
                        "Next depth is predicted to take too long. Stopping."
                    );
                    break;
                }
            }
        }
    }

    // If we started a timer, stop it.
    if let Some(cancel_timer) = cancel_timer {
        cancel_timer.store(true, Ordering::Relaxed);
    }

    analysis
}

fn effective_branching_factor(node_counts: impl Iterator<Item = u64> + Clone) -> f64 {
    let count = (node_counts.clone().count() - 1) as f64;

    let average = node_counts
        .clone()
        .skip(1)
        .zip(node_counts)
        .map(|(n, d)| n as f64 / d as f64)
        .sum::<f64>()
        / count;

    average.sqrt()
}

fn fetch_pv<const N: usize>(
    state: &State<N>,
    tt: &TranspositionTable<N>,
    max_depth: usize,
) -> (Vec<Ply<N>>, State<N>) {
    let mut pv = Vec::new();
    let mut state = state.clone();

    debug!("Fetching PV from transposition table.");

    while let Some(entry) = tt.get(state.metadata.hash) {
        let old_state = state.clone();
        if let Err(err) = state.execute_ply(entry.ply) {
            error!(error = ?err, ?entry, state = ?old_state, "Transposition table ply caused an error. Ending fetch");
            break;
        } else {
            if entry.bound != Bound::Exact {
                warn!(?entry, "Adding non-exact ply.");
            }

            pv.push(entry.ply);

            // Only grab as many as we've actually analyzed.
            if pv.len() == max_depth || state.resolution().is_some() {
                break;
            }
        }
    }

    debug!(?pv, "PV after fetch:");

    if pv.len() < max_depth && state.resolution().is_none() {
        warn!("PV is smaller than search depth!");
    }

    (pv, state)
}

struct SearchState<'a, const N: usize> {
    start_ply: u16,
    stats: DepthStats,
    interrupted: &'a AtomicBool,
    persistent_state: &'a mut PersistentState<N>,
    killer_moves: Vec<KillerMoves<N>>,
    exact_eval: bool,
    evaluator: &'a dyn Evaluator<N>,
}

#[derive(Default)]
struct DepthStats {
    depths: Vec<Statistics>,
}

impl DepthStats {
    fn iter(&self) -> impl Iterator<Item = &Statistics> + Clone {
        self.depths.iter()
    }

    fn depth(&mut self, depth: usize) -> &mut Statistics {
        while self.depths.len() <= depth {
            self.depths.push(Statistics::default());
        }

        &mut self.depths[depth]
    }

    fn total(&self) -> Statistics {
        self.depths.iter().sum()
    }
}

#[derive(Clone, Copy)]
struct BranchResult {
    depth: usize,
    evaluation: Evaluation,
}

impl Neg for BranchResult {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            depth: self.depth,
            evaluation: -self.evaluation,
        }
    }
}

#[instrument(level = "trace", skip_all, fields(rd = remaining_depth, %alpha, %beta, pv_node = alpha.next_up() != beta))]
fn minimax<const N: usize>(
    search: &mut SearchState<'_, N>,
    state: &State<N>,
    remaining_depth: usize,
    mut alpha: Evaluation,
    beta: Evaluation,
    null_move_allowed: bool,
) -> BranchResult {
    let search_depth = (state.ply_count - search.start_ply) as usize;

    search.stats.depth(search_depth).visited += 1;

    let resolution = state.resolution();

    if resolution.is_some() {
        search.stats.depth(search_depth).terminal += 1;
    }

    if remaining_depth == 0 || resolution.is_some() {
        let evaluation = search.evaluator.evaluate(state, resolution);

        trace!(%evaluation, "Leaf");
        search.stats.depth(search_depth).evaluated += 1;

        return BranchResult {
            depth: 0,
            evaluation,
        };
    }

    let pv_node = alpha.next_up() != beta;
    if !pv_node {
        search.stats.depth(search_depth).scouted += 1;
    }

    // Fetch from transposition table ===========

    let mut tt_ply = None;

    if let Some(entry) = search
        .persistent_state
        .transposition_table
        .get(state.metadata.hash)
    {
        search.stats.depth(search_depth).tt_hits += 1;

        let is_save = entry.depth as usize >= remaining_depth
            && match entry.bound {
                Bound::Exact => false, // Search exact nodes to avoid cutting the PV short.
                Bound::Upper => entry.evaluation <= alpha,
                Bound::Lower => entry.evaluation >= beta,
            };

        let is_terminal = entry.bound == Bound::Exact && entry.evaluation.is_terminal();

        if is_save || is_terminal && state.validate_ply(entry.ply).is_ok() {
            search.stats.depth(search_depth).tt_saves += 1;

            return BranchResult {
                depth: entry.depth as usize,
                evaluation: match entry.bound {
                    Bound::Exact => entry.evaluation,
                    Bound::Upper => alpha,
                    Bound::Lower => beta,
                },
            };
        }

        tt_ply = Some(entry.ply);
    }

    // Null move search =========================

    if !search.exact_eval && null_move_allowed && remaining_depth >= 3 {
        let _null_move_span = trace_span!("null_move").entered();
        let mut state = state.clone();

        // Apply a null move.
        state.ply_count += 1;

        let BranchResult { depth, evaluation } = -minimax(
            search,
            &state,
            remaining_depth - 3,
            -beta,
            (-beta).next_up(),
            false,
        );

        // Undo the null move.
        state.ply_count -= 1;

        if evaluation >= beta {
            trace!("Null move cutoff");
            search.stats.depth(search_depth).null_cutoff += 1;

            return BranchResult {
                depth,
                evaluation: beta,
            };
        }
    }

    // Ply search ===============================

    let mut ply_generator =
        PlyGenerator::new(state, tt_ply, search.killer_moves[search_depth].clone());

    let mut best = BranchResult {
        depth: 0,
        evaluation: Evaluation::MIN,
    };
    let mut best_ply = None;

    let mut raised_alpha = false;

    for (i, (fallibility, ply)) in ply_generator.plies().enumerate() {
        let _move_span = trace_span!("move", ?ply).entered();
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

        let next = if i == 0 {
            let _leftmost_span = trace_span!("leftmost").entered();
            // On the first iteration, perform a full-window search.
            -minimax(search, &state, remaining_depth - 1, -beta, -alpha, true)
        } else {
            // Afterwards, perform a null-window search, expecting to fail low (counting
            // on our move ordering to have already led us to the "best" move).
            let scout = -minimax(
                search,
                &state,
                remaining_depth - 1,
                (-alpha).next_down(),
                -alpha,
                true,
            );

            if scout.evaluation > alpha && scout.evaluation < beta {
                trace!(%alpha, %beta, %scout.evaluation, "Researching");
                let _researched_span = trace_span!("researched").entered();
                search.stats.depth(search_depth).re_searched += 1;
                // If we are inside the PV window instead, we need to re-search using the full PV window.
                -minimax(search, &state, remaining_depth - 1, -beta, -alpha, true)
            } else {
                scout
            }
        };

        if next.evaluation > best.evaluation {
            best = next;
            best.depth += 1;

            best_ply = Some((i, ply));
        }

        if next.evaluation > alpha {
            alpha = next.evaluation;
            raised_alpha = true;

            if alpha >= beta {
                alpha = beta;
                search.stats.depth(search_depth).beta_cutoff += 1;

                search.killer_moves[search_depth].push(ply);

                break;
            }
        }

        if search.interrupted.load(Ordering::Relaxed) {
            return BranchResult {
                depth: best.depth,
                evaluation: alpha,
            };
        }
    }

    let (i, best_ply) = best_ply.expect("no plies were searched");

    search.stats.depth(search_depth).best_ply_order[i.min(5)] += 1;

    // Store in transposition table =============

    let inserted = search.persistent_state.transposition_table.insert(
        state.metadata.hash,
        TranspositionTableEntry {
            bound: if alpha == beta {
                Bound::Lower
            } else if raised_alpha {
                Bound::Exact
            } else {
                Bound::Upper
            },
            evaluation: alpha,
            node_count: search
                .stats
                .iter()
                .map(|d| d.visited)
                .sum::<u64>()
                .try_into()
                .unwrap_or(u32::MAX),
            depth: best.depth as u8,
            ply_count: state.ply_count,
            ply: best_ply,
        },
    );

    if inserted {
        search.stats.depth(search_depth).tt_stores += 1;
    } else {
        search.stats.depth(search_depth).tt_store_fails += 1;
    }

    BranchResult {
        depth: best.depth,
        evaluation: alpha,
    }
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
