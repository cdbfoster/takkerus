use tak::Ply;

use crate::plies::Continuation::*;
use crate::plies::Fallibility::*;
use crate::plies::GeneratedPly;

pub(crate) struct TtPly<const N: usize> {
    ply: <Option<Ply<N>> as IntoIterator>::IntoIter,
}

impl<const N: usize> TtPly<N> {
    pub fn new(ply: Option<Ply<N>>) -> Self {
        Self {
            ply: ply.into_iter(),
        }
    }
}

impl<const N: usize> Iterator for TtPly<N> {
    type Item = GeneratedPly<N>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ply.next().map(|ply| GeneratedPly {
            ply,
            fallibility: Fallible,
            continuation: Continue,
        })
    }
}
