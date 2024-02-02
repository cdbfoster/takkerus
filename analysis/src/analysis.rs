use std::fmt::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use tracing::{debug, error, info, trace_span, warn};

use tak::{Ply, State};

use crate::evaluation::{AnnEvaluator, AnnModel, Evaluation, Evaluator};
use crate::search::{minimax, SearchState};
use crate::statistics::{AtomicStatistics, Statistics};
use crate::time::TimeControl;
use crate::transposition_table::{Bound, TranspositionTable};
use crate::util::Sender;

pub struct AnalysisConfig<'a, const N: usize> {
    pub depth_limit: Option<u32>,
    pub time_limit: Option<Duration>,
    pub early_stop: bool,
    pub time_control: Option<TimeControl>,
    pub interrupted: Arc<AtomicBool>,
    /// A place to put data gathered during the search that could be
    /// useful to future searches. If none, this will be created internally.
    pub persistent_state: Option<&'a PersistentState<N>>,
    /// If false, the search is allowed to use unprovable methods that may
    /// improve playing strength at the cost of correctness.
    pub exact_eval: bool,
    /// The evaluator to use for the search. If none, a default will be
    /// used internally.
    pub evaluator: Option<&'a dyn Evaluator<N>>,
    /// A sender that will be used to send interim results during the search.
    pub interim_analysis_sender: Option<Box<dyn Sender<Analysis<N>>>>,
    /// The number of threads to use during the search.
    pub threads: usize,
}

impl<'a, const N: usize> Default for AnalysisConfig<'a, N> {
    fn default() -> Self {
        Self {
            depth_limit: Default::default(),
            time_limit: Default::default(),
            early_stop: Default::default(),
            time_control: Default::default(),
            interrupted: Default::default(),
            persistent_state: Default::default(),
            exact_eval: Default::default(),
            evaluator: Default::default(),
            interim_analysis_sender: Default::default(),
            threads: 1,
        }
    }
}

#[derive(Debug)]
pub struct PersistentState<const N: usize> {
    pub(crate) transposition_table: TranspositionTable<N>,
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
    pub state: State<N>,
    pub depth: u32,
    pub final_state: State<N>,
    pub evaluation: Evaluation,
    pub principal_variation: Vec<Ply<N>>,
    pub stats: Statistics,
    pub time: Duration,
}

/// Analyzes a position given a configuration, and returns an evaluation and principal variation.
pub fn analyze<const N: usize>(config: AnalysisConfig<N>, state: &State<N>) -> Analysis<N> {
    info!(
        "Analyzing... depth_limit: {}, time_limit: {}, early_stop: {:?}",
        if let Some(depth_limit) = config.depth_limit {
            depth_limit.to_string()
        } else {
            "none".to_owned()
        },
        if let Some(time_limit) = config.time_limit {
            format!("{:.2}s", time_limit.as_secs_f32())
        } else {
            "none".to_owned()
        },
        config.early_stop,
    );

    if let Some(tc) = config.time_control {
        info!("Using time control: {tc}");
    }

    let time_limit = match (
        config.time_limit,
        config.time_control.map(|tc| tc.get_use_time(state)),
    ) {
        (Some(maximum_time), Some(use_time)) => Some(maximum_time.min(use_time)),
        (Some(maximum_time), None) => Some(maximum_time),
        (None, Some(use_time)) => Some(use_time),
        (None, None) => None,
    };

    let interrupt = time_limit.map(|time_limit| spawn_interrupt_thread(&config, time_limit));

    let max_depth = config.depth_limit.unwrap_or(u32::MAX) as usize;

    // Use the passed-in persistent state or create a local one for this analysis.
    let local_persistent_state;
    let persistent_state = if let Some(persistent_state) = config.persistent_state {
        persistent_state
    } else {
        local_persistent_state = PersistentState::default();
        &local_persistent_state
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
        state: state.clone(),
        depth: 0,
        final_state: state.clone(),
        evaluation: evaluator.evaluate(state, state.resolution()),
        principal_variation: Vec::new(),
        stats: Statistics::default(),
        time: Duration::ZERO,
    };

    let search_start_time = Instant::now();

    let mut iteration_times = Vec::new();

    for iteration in 1..=max_depth {
        let iteration_start_time = Instant::now();

        let depth_stats = AtomicStatistics::default();
        let workers_terminated = AtomicBool::default();

        let search = SearchState {
            start_ply: state.ply_count,
            stats: &depth_stats,
            interrupted: &config.interrupted,
            workers_terminated: &workers_terminated,
            persistent_state,
            killer_moves: Default::default(),
            exact_eval: config.exact_eval,
            evaluator,
        };

        debug!(iteration, "Beginning analysis...");

        let root = thread::scope(|scope| {
            for i in 1..config.threads {
                let mut search = search.clone();
                let worker_depth = iteration + i;

                thread::Builder::new()
                    .name(format!("worker_{i}"))
                    .spawn_scoped(scope, move || {
                        let _worker_thread =
                            trace_span!("thread", id = %thread::current().name().unwrap())
                                .entered();

                        let _ = minimax(
                            &mut search,
                            &state.clone(),
                            worker_depth,
                            Evaluation::MIN,
                            Evaluation::MAX,
                            true,
                        );
                    })
                    .expect("could not spawn worker thread");
            }

            let _main_thread = trace_span!("thread", id = %"main").entered();

            let mut search = search.clone();
            let root = minimax(
                &mut search,
                state,
                iteration,
                Evaluation::MIN,
                Evaluation::MAX,
                true,
            );
            search.workers_terminated.store(true, Ordering::Relaxed);

            root
        });

        if config.interrupted.load(Ordering::Relaxed) {
            break;
        }

        let (principal_variation, final_state) = fetch_pv(
            state,
            &search.persistent_state.transposition_table,
            root.depth,
        );

        let search_stats = search.stats.load();

        analysis = Analysis {
            state: analysis.state,
            depth: root.depth as u32,
            final_state,
            evaluation: root.evaluation,
            principal_variation,
            stats: &analysis.stats + &search_stats,
            time: search_start_time.elapsed(),
        };

        if let Some(sender) = &config.interim_analysis_sender {
            if let Err(error) = sender.send(analysis.clone()) {
                error!(?error, "Could not send interim analysis.");
            }
        }

        let iteration_time = iteration_start_time.elapsed();

        info!(
            depth = analysis.depth,
            time = %format!("{:05.2}s", analysis.time.as_secs_f64()),
            eval = %format!("{:<4}", analysis.evaluation),
            pv = ?analysis.principal_variation,
            "Analyzed:",
        );

        debug!(
            visited = search_stats.visited,
            evaluated = search_stats.evaluated,
            terminal = search_stats.terminal,
            scouted = search_stats.scouted,
            re_searched = search_stats.re_searched,
            beta_cutoff = search_stats.beta_cutoff,
            null_cutoff = search_stats.null_cutoff,
            tt_stores = search_stats.tt_stores,
            tt_store_fails = search_stats.tt_store_fails,
            tt_hits = search_stats.tt_hits,
            tt_saves = search_stats.tt_saves,
            tt_full = %format!(
                "{:05.2}%",
                100.0 * search.persistent_state.transposition_table.len() as f64
                    / search.persistent_state.transposition_table.capacity() as f64
            ),
            "Stats:",
        );

        // Best ply ordering calc.
        {
            fn order_string(values: &[u64; 6]) -> String {
                let total = values.iter().sum::<u64>().max(1);
                let mut buffer = String::new();
                for (i, &c) in values.iter().enumerate() {
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
                buffer
            }

            debug!(
                "PV-Node best-ply ordering: {}",
                order_string(&search_stats.pv_ply_order)
            );
            debug!(
                "All-Node best-ply ordering: {}",
                order_string(&search_stats.all_ply_order)
            );
        }

        iteration_times.push(iteration_time.as_secs_f64());

        // Time factors are just the current iteration time divided by the previous iteration time.
        let mut time_factors = iteration_times
            .iter()
            .copied()
            .zip(std::iter::once(iteration_times[0]).chain(iteration_times.iter().copied()))
            .map(|(n, d)| n / d);

        // Calculate the average time factor separately for even/odd iterations.
        let time_factor = 2.0
            * if iteration_times.len() == 1 {
                time_factors.next().unwrap()
            } else if iteration % 2 == 1 {
                time_factors.skip(1).step_by(2).sum::<f64>() / iteration_times.len() as f64
            } else {
                time_factors.step_by(2).sum::<f64>() / iteration_times.len() as f64
            };

        let next_iteration_prediction = iteration_time.as_secs_f64() * time_factor;

        debug!(
            time_factor = %format!("{:.2}", time_factor),
            rate = %format!("{}n/s", (search_stats.visited as f64 / iteration_time.as_secs_f64()) as u64),
            next_iteration_prediction = %format!("{:.2}s", analysis.time.as_secs_f64() + next_iteration_prediction),
            "Search:",
        );

        if analysis.evaluation.is_terminal() {
            info!("Tinuë found. Stopping.");
            break;
        }

        if let Some(time_limit) = config.time_limit {
            if config.early_stop
                && analysis.time + Duration::from_secs_f64(next_iteration_prediction) > time_limit
            {
                info!(
                    time = %format!("{:.2}s", analysis.time.as_secs_f64()),
                    limit = %format!("{:.2}s", time_limit.as_secs_f64()),
                    prediction = %format!("{:.2}s", analysis.time.as_secs_f64() + next_iteration_prediction),
                    "Next iteration is predicted to take too long. Stopping."
                );
                break;
            }
        }
    }

    // If we started an interrupt timer, stop it.
    if let Some(interrupt) = interrupt {
        interrupt.cancel();
    }

    analysis
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
        if let Err(err) = state.execute_ply(entry.ply()) {
            error!(error = ?err, ?entry, state = ?old_state, "Transposition table ply caused an error. Ending fetch");
            break;
        } else {
            if entry.bound() != Bound::Exact {
                warn!(?entry, "Adding non-exact ply.");
            }

            pv.push(entry.ply());

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

fn spawn_interrupt_thread<const N: usize>(
    config: &AnalysisConfig<N>,
    time_limit: Duration,
) -> InterruptHandle {
    let cancel = Arc::new(AtomicBool::new(false));
    let start_time = Instant::now();

    {
        let cancel = cancel.clone();
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

    InterruptHandle(cancel)
}

struct InterruptHandle(Arc<AtomicBool>);

impl InterruptHandle {
    fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
