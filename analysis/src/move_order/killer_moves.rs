use crate::plies::Continuation::*;
use crate::plies::Fallibility::*;
use crate::plies::GeneratedPly;
use crate::search::KillerMoves;

pub(crate) struct Killers<const N: usize> {
    killer_moves: KillerMoves<N>,
}

impl<const N: usize> Killers<N> {
    pub fn new(killer_moves: &KillerMoves<N>) -> Self {
        Self {
            killer_moves: killer_moves.clone(),
        }
    }
}

impl<const N: usize> Iterator for Killers<N> {
    type Item = GeneratedPly<N>;

    fn next(&mut self) -> Option<Self::Item> {
        self.killer_moves.pop().map(|ply| GeneratedPly {
            ply,
            fallibility: Fallible,
            continuation: Continue,
        })
    }
}
