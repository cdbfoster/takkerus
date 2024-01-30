use std::collections::HashSet;
use std::iter::Chain;

use fnv::FnvBuildHasher;

use tak::{Ply, State};

use crate::move_order::{AllPlies, KillerMoves, Killers, PlacementWins, TtPly};

#[derive(PartialEq)]
pub(crate) enum Fallibility {
    Fallible,
    Infallible,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Continuation {
    Continue,
    Stop,
}
use Continuation::*;

pub(crate) struct PlyGenerator<'a, const N: usize> {
    used_plies: HashSet<Ply<N>, FnvBuildHasher>,
    plies: PlyIterator<'a, N>,
    continuation: Continuation,
}

impl<'a, const N: usize> PlyGenerator<'a, N> {
    pub(crate) fn new(
        state: &'a State<N>,
        tt_ply: Option<Ply<N>>,
        killer_moves: &KillerMoves<N>,
    ) -> Self {
        Self {
            used_plies: HashSet::default(),
            plies: PlacementWins::new(state)
                .chain(TtPly::new(tt_ply))
                .chain(Killers::new(killer_moves))
                .chain(AllPlies::new(state)),
            continuation: Continue,
        }
    }
}

type PlyIterator<'a, const N: usize> =
    Chain<Chain<Chain<PlacementWins<'a, N>, TtPly<N>>, Killers<N>>, AllPlies<'a, N>>;

impl<'a, const N: usize> Iterator for PlyGenerator<'a, N> {
    type Item = (Fallibility, Ply<N>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.continuation == Stop {
            return None;
        }

        'generate: loop {
            if let Some(next_ply) = self.plies.next() {
                if !self.used_plies.insert(next_ply.ply) {
                    // We've already seen this ply, so get another.
                    continue 'generate;
                }
                self.continuation = next_ply.continuation;
                return Some((next_ply.fallibility, next_ply.ply));
            } else {
                return None;
            }
        }
    }
}
