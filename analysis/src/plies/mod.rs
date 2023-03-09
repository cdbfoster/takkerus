use std::cell::RefCell;
use std::iter::Chain;

use tak::{Ply, State};

pub(crate) use self::order::KillerMoves;
use self::order::{AllPlies, Killers, PlacementWins, TtPly};

pub mod generation;
mod order;

use Continuation::*;

#[derive(PartialEq)]
pub(crate) enum Fallibility {
    Fallible,
    Infallible,
}

#[derive(Clone, Copy, PartialEq)]
enum Continuation {
    Continue,
    Stop,
}

type PlyBuffer<const N: usize> = RefCell<Vec<Ply<N>>>;

pub(crate) struct PlyGenerator<'a, const N: usize> {
    used_plies: PlyBuffer<N>,
    state: &'a State<N>,
    tt_ply: Option<Ply<N>>,
    killer_moves: KillerMoves<N>,
}

impl<'a, const N: usize> PlyGenerator<'a, N> {
    pub(crate) fn new(
        state: &'a State<N>,
        tt_ply: Option<Ply<N>>,
        killer_moves: KillerMoves<N>,
    ) -> Self {
        Self {
            used_plies: PlyBuffer::default(),
            state,
            tt_ply,
            killer_moves,
        }
    }

    pub(crate) fn plies(&mut self) -> PlyGeneratorIter<'_, N> {
        PlyGeneratorIter::new(self)
    }
}

pub(crate) struct PlyGeneratorIter<'a, const N: usize> {
    used_plies: &'a PlyBuffer<N>,
    plies: PlyIterator<'a, N>,
    continuation: Continuation,
}

type PlyIterator<'a, const N: usize> =
    Chain<Chain<Chain<PlacementWins<'a, N>, TtPly<'a, N>>, Killers<'a, N>>, AllPlies<'a, N>>;

impl<'a, const N: usize> PlyGeneratorIter<'a, N> {
    fn new(generator: &'a mut PlyGenerator<N>) -> Self {
        let plies = PlacementWins {
            state: generator.state,
        }
        .chain(TtPly {
            used_plies: &generator.used_plies,
            ply: generator.tt_ply,
        })
        .chain(Killers {
            used_plies: &generator.used_plies,
            killer_moves: &mut generator.killer_moves,
        })
        .chain(AllPlies {
            used_plies: &generator.used_plies,
            state: generator.state,
            plies: None,
        });

        Self {
            used_plies: &generator.used_plies,
            plies,
            continuation: Continue,
        }
    }
}

impl<'a, const N: usize> Iterator for PlyGeneratorIter<'a, N> {
    type Item = (Fallibility, Ply<N>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.continuation == Stop {
            return None;
        }

        let next_ply = self.plies.next();

        if let Some(generated_ply) = &next_ply {
            let mut used_plies = self.used_plies.borrow_mut();
            used_plies.push(generated_ply.ply);

            self.continuation = generated_ply.continuation;
        }

        next_ply.map(|generated_ply| (generated_ply.fallibility, generated_ply.ply))
    }
}
