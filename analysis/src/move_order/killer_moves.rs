use crate::ply_generator::Continuation::*;
use crate::ply_generator::Fallibility::*;
use crate::ply_generator::GeneratedPly;
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
