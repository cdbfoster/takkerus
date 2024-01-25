use std::iter::Enumerate;
use std::mem;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use tracing::{instrument, trace, trace_span, warn};

use tak::{Ply, State};

use crate::evaluation::Evaluation;
use crate::plies::{Fallibility, PlyGenerator};
use crate::transposition_table::{Bound, TranspositionTableEntry};
use crate::util::bag::Bag;
use crate::util::Neighbors;

use super::work::{WorkNode, WorkNodeResults, WorkNodeStatus, WorkNodeVariables, WorkNodes};
use super::{BranchResult, Node, SearchState};

#[instrument(
    level = "trace",
    skip_all,
    fields(
        rd = node.remaining_depth,
        alpha = %alpha,
        beta = %beta,
        pv_node = alpha.next_up() != beta,
    ),
)]
pub(super) fn ab_search<const N: usize>(
    search: &SearchState<'_, N>,
    node: Node<N>,
    work: &WorkNodes<N>,
    mut alpha: Evaluation,
    beta: Evaluation,
) -> BranchResult<N> {
    let search_depth = (node.state.ply_count - search.start_ply) as usize;

    search.stats.visited.fetch_add(1, Ordering::Relaxed);

    let pv_node = alpha.next_up() != beta;
    if !pv_node {
        search.stats.scouted.fetch_add(1, Ordering::Relaxed);
    }

    // Evaluate leaves ==========================

    let resolution = node.state.resolution();

    if resolution.is_some() {
        search.stats.terminal.fetch_add(1, Ordering::Relaxed);
    }

    if node.remaining_depth == 0 || resolution.is_some() {
        let evaluation = search.evaluator.evaluate(&node.state, resolution);

        trace!(%evaluation, "Leaf");
        search.stats.evaluated.fetch_add(1, Ordering::Relaxed);

        return BranchResult {
            best_ply: None,
            depth: 0,
            evaluation,
        };
    }

    // Fetch from transposition table ===========

    let tt_ply = match fetch_from_tt(search, &node, alpha, beta) {
        TtHit::None => None,
        TtHit::Move(ply) => Some(ply),
        TtHit::Save(result) => return result,
    };

    // Null move search =========================

    if can_use_null_move_search(search, &node) {
        if let NullMoveResult::Cutoff(result) = null_move_search(search, &node, work, beta) {
            return result;
        }
    }

    // Ply search ===============================

    let ply_generator =
        PlyGenerator::new(&node.state, tt_ply, search.killer_moves.depth(search_depth)).enumerate();

    let results = Arc::new(WorkNodeResults::default());

    let work_node = WorkNode {
        ply_generator,
        node: node.clone(),
        variables: WorkNodeVariables {
            status: WorkNodeStatus::Wait,
            alpha,
            beta,
        },
        results: results.clone(),
    };

    let (index, leftmost_move_order, leftmost_ply, leftmost_state) = {
        // Publish the work to the pile. Other threads will leave it alone until
        // its status becomes Active. Get the index so we can refer back to it in
        // this thread.
        let mut work_nodes = work.lock();
        let index = work_nodes.push(work_node);

        // Get the leftmost child: https://www.chessprogramming.org/Young_Brothers_Wait_Concept
        let work_node = work_nodes.get_mut(index).expect("work node disappeared");
        let (move_order, ply, state) =
            advance_state(&node.state, &mut work_node.ply_generator).expect("no plies to search");
        (index, move_order, ply, state)
    };

    let leftmost = {
        let _move_span = trace_span!("move", ply = ?leftmost_ply).entered();
        let _leftmost_span = trace_span!("leftmost").entered();
        -ab_search(
            search,
            Node {
                parent_index: Some(index),
                state: leftmost_state,
                remaining_depth: node.remaining_depth - 1,
                null_move_allowed: true,
            },
            work,
            -beta,
            -alpha,
        )
    };

    let mut results_lock = results.lock();
    results_lock.push((
        leftmost_move_order,
        BranchResult {
            best_ply: Some(leftmost_ply),
            depth: leftmost.depth + 1,
            ..leftmost
        },
    ));

    let mut best = BranchResult {
        best_ply: None,
        depth: 0,
        evaluation: Evaluation::MIN,
    };
    let mut best_move_order = 0;
    let mut raised_alpha = false;
    let mut results_cursor = 0;

    macro_rules! process_results {
        ($results_lock: ident, drop = $drop: tt) => {{
            let mut update_bounds = false;
            let mut prune = false;

            for &(move_order, next) in &$results_lock[results_cursor..] {
                if next.evaluation > best.evaluation {
                    best = next;
                    best_move_order = move_order;
                }

                if next.evaluation > alpha {
                    alpha = next.evaluation;
                    raised_alpha = true;
                    update_bounds = true;

                    if alpha >= beta {
                        alpha = beta;
                        prune = true;
                        search.stats.beta_cutoff.fetch_add(1, Ordering::Relaxed);
                        search
                            .killer_moves
                            .depth(search_depth)
                            .push(next.best_ply.expect("no best ply"));
                        break;
                    }
                }
            }
            results_cursor = $results_lock.len();
            process_results!($drop, $results_lock);

            if search.interrupted.load(Ordering::Relaxed) {
                return BranchResult {
                    evaluation: alpha,
                    ..best
                };
            }

            if prune {
                {
                    let mut work_nodes = work.lock();
                    prune_children(&mut work_nodes, index);
                    work_nodes.remove(index);
                }

                store_in_tt(
                    search,
                    &node,
                    alpha,
                    beta,
                    best.best_ply.expect("no best ply"),
                    best.depth.max(node.remaining_depth),
                    raised_alpha,
                );

                return BranchResult {
                    evaluation: alpha,
                    ..best
                };
            }

            if update_bounds {
                let mut work_nodes = work.lock();
                let work_node = work_nodes.get_mut(index).expect("work node disappeared");
                work_node.variables.alpha = alpha;
                work_node.variables.beta = beta;
            }
        }};
        (true, $results_lock: ident) => {
            mem::drop($results_lock);
        };
        (false, $results_lock: ident) => {};
    }
    process_results!(results_lock, drop = true);

    // Now that we've searched the leftmost node, change the work node's status,
    // allowing other threads to work on it.
    {
        let mut work_nodes = work.lock();
        let work_node = work_nodes.get_mut(index).expect("work node disappeared");
        work_node.variables.status = WorkNodeStatus::Active;
        work.queue.notify_all();
    }

    'next_ply: loop {
        let (move_order, ply, state) = {
            let mut work_nodes = work.lock();
            let work_node = work_nodes.get_mut(index).expect("work node disappeared");

            if work_node.variables.status == WorkNodeStatus::Pruned {
                // Some ancestor has been pruned; just return.
                return BranchResult {
                    evaluation: alpha,
                    ..best
                };
            }

            match advance_state(&node.state, &mut work_node.ply_generator) {
                Some((move_order, ply, state)) => (move_order, ply, state),
                None => break 'next_ply,
            }
        };

        let _move_span = trace_span!("move", ?ply).entered();

        let next = pvs(
            search,
            Node {
                parent_index: Some(index),
                state,
                remaining_depth: node.remaining_depth - 1,
                null_move_allowed: true,
            },
            work,
            alpha,
            beta,
        );

        let mut results_lock = results.lock();
        results_lock.push((
            move_order,
            BranchResult {
                best_ply: Some(ply),
                depth: next.depth + 1,
                ..next
            },
        ));

        process_results!(results_lock, drop = true);
    }

    // The generator is exhausted, so all plies have either been searched
    // or are in progress.
    {
        let mut results_lock = results.lock();
        process_results!(results_lock, drop = false);

        trace!(workers = results_lock.workers());
        while results_lock.workers() > 1 {
            // There must still be at least one thread working on a child of
            // this node. Wait until they finish and notify us.
            results_lock = results.queue.wait(results_lock).unwrap();

            process_results!(results_lock, drop = false);
        }
        trace!("all workers finished");
    }

    // All plies have been searched and all results have been processed.
    {
        let mut work_nodes = work.lock();
        work_nodes.remove(index);
    }

    if raised_alpha {
        // PV nodes
        search.stats.pv_ply_order[best_move_order.min(5)].fetch_add(1, Ordering::Relaxed);
    } else {
        // Fail-low nodes
        search.stats.all_ply_order[best_move_order.min(5)].fetch_add(1, Ordering::Relaxed);
    }

    store_in_tt(
        search,
        &node,
        alpha,
        beta,
        best.best_ply.expect("no best ply"),
        best.depth.max(node.remaining_depth),
        raised_alpha,
    );

    return BranchResult {
        evaluation: alpha,
        ..best
    };
}

enum TtHit<const N: usize> {
    None,
    Move(Ply<N>),
    Save(BranchResult<N>),
}

fn fetch_from_tt<const N: usize>(
    search: &SearchState<'_, N>,
    node: &Node<N>,
    alpha: Evaluation,
    beta: Evaluation,
) -> TtHit<N> {
    let entry = match search
        .persistent_state
        .transposition_table
        .get(node.state.metadata.hash)
    {
        None => return TtHit::None,
        Some(entry) => entry,
    };

    search.stats.tt_hits.fetch_add(1, Ordering::Relaxed);

    let is_save = entry.depth() >= node.remaining_depth
        && match entry.bound() {
            Bound::Exact => false, // Search exact nodes to avoid cutting the PV short.
            Bound::Upper => entry.evaluation() <= alpha,
            Bound::Lower => entry.evaluation() >= beta,
        };

    let is_terminal = entry.bound() == Bound::Exact && entry.evaluation().is_terminal();

    if is_save || is_terminal && node.state.validate_ply(entry.ply()).is_ok() {
        search.stats.tt_saves.fetch_add(1, Ordering::Relaxed);

        TtHit::Save(BranchResult {
            best_ply: Some(entry.ply()),
            depth: entry.depth(),
            evaluation: match entry.bound() {
                Bound::Exact => entry.evaluation(),
                Bound::Upper => alpha,
                Bound::Lower => beta,
            },
        })
    } else {
        TtHit::Move(entry.ply())
    }
}

fn store_in_tt<const N: usize>(
    search: &SearchState<'_, N>,
    node: &Node<N>,
    alpha: Evaluation,
    beta: Evaluation,
    ply: Ply<N>,
    depth: usize,
    raised_alpha: bool,
) {
    let bound = if alpha == beta {
        Bound::Lower
    } else if raised_alpha {
        Bound::Exact
    } else {
        Bound::Upper
    };

    let inserted = search.persistent_state.transposition_table.insert(
        node.state.metadata.hash,
        TranspositionTableEntry::new(
            ply,
            alpha,
            bound,
            depth.max(node.remaining_depth),
            node.state.ply_count,
        ),
    );

    if inserted {
        search.stats.tt_stores.fetch_add(1, Ordering::Relaxed);
    } else {
        search.stats.tt_store_fails.fetch_add(1, Ordering::Relaxed);
    }
}

fn can_use_null_move_search<const N: usize>(search: &SearchState<'_, N>, node: &Node<N>) -> bool {
    !search.exact_eval && node.null_move_allowed && node.remaining_depth >= 3
}

enum NullMoveResult<const N: usize> {
    None,
    Cutoff(BranchResult<N>),
}

#[instrument(name = "null_move", level = "trace", skip_all)]
fn null_move_search<const N: usize>(
    search: &SearchState<'_, N>,
    node: &Node<N>,
    work: &WorkNodes<N>,
    beta: Evaluation,
) -> NullMoveResult<N> {
    let mut state = node.state.clone();

    // Apply a null move.
    state.ply_count += 1;

    let scout = -ab_search(
        search,
        Node {
            parent_index: node.parent_index,
            state,
            remaining_depth: node.remaining_depth - 3,
            null_move_allowed: false,
        },
        work,
        -beta,
        (-beta).next_up(),
    );

    if scout.evaluation >= beta {
        trace!("Null move cutoff");
        search.stats.null_cutoff.fetch_add(1, Ordering::Relaxed);

        NullMoveResult::Cutoff(BranchResult {
            evaluation: beta,
            ..scout
        })
    } else {
        NullMoveResult::None
    }
}

fn advance_state<const N: usize>(
    state: &State<N>,
    ply_generator: &mut Enumerate<PlyGenerator<N>>,
) -> Option<(usize, Ply<N>, State<N>)> {
    let mut state = state.clone();

    for (i, (fallibility, ply)) in ply_generator.by_ref() {
        match fallibility {
            Fallibility::Fallible => {
                if state.execute_ply(ply).is_err() {
                    trace!(?ply, "attempting");
                    continue;
                }
            }
            Fallibility::Infallible => state.execute_ply_unchecked(ply),
        }

        return Some((i, ply, state));
    }

    None
}

fn prune_children<const N: usize>(work_nodes: &mut Bag<WorkNode<N>>, parent_index: usize) {
    for i in 0..work_nodes.len() {
        if let Some(work_node) = work_nodes.get_mut(i) {
            if work_node.node.parent_index == Some(parent_index) {
                work_node.variables.status = WorkNodeStatus::Pruned;
                prune_children(work_nodes, i);
            }
        }
    }
}

pub(super) fn pvs<const N: usize>(
    search: &SearchState<'_, N>,
    node: Node<N>,
    work: &WorkNodes<N>,
    alpha: Evaluation,
    beta: Evaluation,
) -> BranchResult<N> {
    // Perform a null-window search, expecting to fail low (counting
    // on our move ordering to have already led us to the "best" move).
    let scout = -ab_search(search, node.clone(), work, (-alpha).next_down(), -alpha);

    if scout.evaluation > alpha && scout.evaluation < beta {
        // If we are inside the PV window instead, we need to re-search using the full PV window.

        trace!(%alpha, %beta, %scout.evaluation, "Researching");
        let _researched_span = trace_span!("researched").entered();
        search.stats.re_searched.fetch_add(1, Ordering::Relaxed);

        -ab_search(search, node, work, -beta, -alpha)
    } else {
        scout
    }
}

pub(super) fn worker<const N: usize>(search: &SearchState<'_, N>, work: &WorkNodes<N>) {
    'ingest_work: loop {
        let mut work_nodes = work.lock();

        let work_node = 'find_work: loop {
            if work.shutdown.load(Ordering::Relaxed) {
                break 'ingest_work;
            }

            let active_node = work_nodes
                .find_index(|work_node| work_node.variables.status == WorkNodeStatus::Active);

            if let Some(index) = active_node {
                break 'find_work work_nodes.get_mut(index).expect("work node disappeared");
            } else {
                work_nodes = work.queue.wait(work_nodes).unwrap();
            }
        };

        let results = work_node.results.clone();
        let parent_index = work_node.node.parent_index;
        let remaining_depth = work_node.node.remaining_depth - 1;
        let alpha = work_node.variables.alpha;
        let beta = work_node.variables.beta;

        let (move_order, ply, state) =
            match advance_state(&work_node.node.state, &mut work_node.ply_generator) {
                Some((move_order, ply, state)) => (move_order, ply, state),
                None => continue 'ingest_work,
            };

        mem::drop(work_nodes);

        let _move_span = trace_span!("move", ?ply).entered();

        {
            let mut results_lock = results.lock();
            results_lock.inc_workers();
            trace!(workers = results_lock.workers(), "increasing workers");
        }

        let result = pvs(
            search,
            Node {
                parent_index,
                state,
                remaining_depth,
                null_move_allowed: true,
            },
            work,
            alpha,
            beta,
        );

        {
            let mut results_lock = results.lock();
            results_lock.push((
                move_order,
                BranchResult {
                    best_ply: Some(ply),
                    depth: result.depth + 1,
                    ..result
                },
            ));
            results_lock.dec_workers();
            trace!(workers = results_lock.workers(), "decreasing workers");
            results.queue.notify_one();
        }
    }
}
