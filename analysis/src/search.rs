use std::mem;
use std::ops::Neg;
use std::sync::atomic::{AtomicBool, Ordering};

use tracing::{error, instrument, trace, trace_span, warn};

use tak::{zobrist_advance_move, Ply, State};

use crate::analysis::PersistentState;
use crate::evaluation::{Evaluation, Evaluator};
use crate::ply_generator::{Fallibility, PlyGenerator};
use crate::statistics::AtomicStatistics;
use crate::transposition_table::{Bound, TranspositionTableEntry};
use crate::util::Neighbors;

pub(crate) struct SearchState<'a, const N: usize> {
    pub start_ply: u16,
    pub stats: AtomicStatistics,
    pub interrupted: &'a AtomicBool,
    pub persistent_state: &'a PersistentState<N>,
    pub killer_moves: DepthKillerMoves<N>,
    pub exact_eval: bool,
    pub evaluator: &'a dyn Evaluator<N>,
}

#[derive(Default)]
pub(crate) struct DepthKillerMoves<const N: usize> {
    depths: Vec<KillerMoves<N>>,
}

impl<const N: usize> DepthKillerMoves<N> {
    pub fn depth(&mut self, depth: usize) -> &mut KillerMoves<N> {
        while self.depths.len() <= depth {
            self.depths.push(KillerMoves::default());
        }

        &mut self.depths[depth]
    }
}

#[derive(Clone, Default)]
pub(crate) struct KillerMoves<const N: usize> {
    start: usize,
    buffer: [Option<Ply<N>>; 2],
}

impl<const N: usize> KillerMoves<N> {
    pub fn push(&mut self, ply: Ply<N>) {
        if !self.buffer.contains(&Some(ply)) {
            self.start = (self.start + 1) % self.buffer.len();
            self.buffer[self.start] = Some(ply);
        }
    }

    pub fn pop(&mut self) -> Option<Ply<N>> {
        let ply = mem::take(&mut self.buffer[self.start]);
        self.start = match self.start > 0 {
            true => self.start - 1,
            false => self.buffer.len() - 1,
        };
        ply
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BranchResult {
    pub depth: usize,
    pub evaluation: Evaluation,
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

/// Computes the minimax function with common enhancements: alpha-beta pruning,
/// principal variation search, transposition table, null move pruning, killer moves.
#[instrument(level = "trace", skip_all, fields(rd = remaining_depth, %alpha, %beta, pv_node = alpha.next_up() != beta))]
pub(crate) fn minimax<const N: usize>(
    search: &mut SearchState<'_, N>,
    state: &State<N>,
    remaining_depth: usize,
    mut alpha: Evaluation,
    beta: Evaluation,
    null_move_allowed: bool,
) -> BranchResult {
    let search_depth = (state.ply_count - search.start_ply) as usize;

    search.stats.visited.fetch_add(1, Ordering::Relaxed);

    let resolution = state.resolution();

    if resolution.is_some() {
        search.stats.terminal.fetch_add(1, Ordering::Relaxed);
    }

    if remaining_depth == 0 || resolution.is_some() {
        let evaluation = search.evaluator.evaluate(state, resolution);

        trace!(%evaluation, "Leaf");
        search.stats.evaluated.fetch_add(1, Ordering::Relaxed);

        return BranchResult {
            depth: 0,
            evaluation,
        };
    }

    let pv_node = alpha.next_up() != beta;
    if !pv_node {
        search.stats.scouted.fetch_add(1, Ordering::Relaxed);
    }

    // Fetch from transposition table ===========

    let tt_entry = search
        .persistent_state
        .transposition_table
        .get(state.metadata.hash);

    if let Some(entry) = tt_entry {
        search.stats.tt_hits.fetch_add(1, Ordering::Relaxed);

        let is_save = entry.depth() >= remaining_depth
            && match entry.bound() {
                Bound::Exact => true,
                Bound::Upper => entry.evaluation() <= alpha,
                Bound::Lower => entry.evaluation() >= beta,
            };

        let is_terminal = entry.bound() == Bound::Exact && entry.evaluation().is_terminal();

        #[cfg(debug_assertions)]
        if let Err(err) = state.validate_ply(entry.ply()) {
            error!(?entry, ?state, error = ?err, "Invalid tt ply");
        }

        if is_save || is_terminal {
            search.stats.tt_saves.fetch_add(1, Ordering::Relaxed);

            return BranchResult {
                depth: entry.depth(),
                evaluation: match entry.bound() {
                    Bound::Exact => entry.evaluation(),
                    Bound::Upper => alpha,
                    Bound::Lower => beta,
                },
            };
        }
    }

    // Null move search =========================

    if !search.exact_eval && null_move_allowed && remaining_depth >= 3 {
        let _null_move_span = trace_span!("null_move").entered();
        let mut state = state.clone();

        // Apply a null move.
        state.ply_count += 1;
        state.metadata.hash ^= zobrist_advance_move::<N>();

        let BranchResult { depth, evaluation } = -minimax(
            search,
            &state,
            remaining_depth - 3,
            -beta,
            (-beta).next_up(),
            false,
        );

        if evaluation >= beta {
            trace!("Null move cutoff");
            search.stats.null_cutoff.fetch_add(1, Ordering::Relaxed);

            return BranchResult {
                depth,
                evaluation: beta,
            };
        }
    }

    // Ply search ===============================

    let ply_generator = PlyGenerator::new(
        state,
        tt_entry.map(|entry| entry.ply()),
        search.killer_moves.depth(search_depth),
    );

    let mut best = BranchResult {
        depth: 0,
        evaluation: Evaluation::MIN,
    };
    let mut best_ply = None;

    let mut raised_alpha = false;

    for (i, (fallibility, ply)) in ply_generator.enumerate() {
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
                search.stats.re_searched.fetch_add(1, Ordering::Relaxed);
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
                search.stats.beta_cutoff.fetch_add(1, Ordering::Relaxed);

                search.killer_moves.depth(search_depth).push(ply);

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

    let bound = if alpha == beta {
        Bound::Lower
    } else if raised_alpha {
        Bound::Exact
    } else {
        Bound::Upper
    };

    if bound == Bound::Exact {
        search.stats.pv_ply_order[i.min(5)].fetch_add(1, Ordering::Relaxed);
    } else if bound == Bound::Upper {
        search.stats.all_ply_order[i.min(5)].fetch_add(1, Ordering::Relaxed);
    }

    // Store in transposition table =============

    let inserted = search.persistent_state.transposition_table.insert(
        state.metadata.hash,
        TranspositionTableEntry::new(
            best_ply,
            alpha,
            bound,
            best.depth.max(remaining_depth),
            state.ply_count,
        ),
    );

    if inserted {
        search.stats.tt_stores.fetch_add(1, Ordering::Relaxed);
    } else {
        search.stats.tt_store_fails.fetch_add(1, Ordering::Relaxed);
    }

    BranchResult {
        depth: best.depth,
        evaluation: alpha,
    }
}
