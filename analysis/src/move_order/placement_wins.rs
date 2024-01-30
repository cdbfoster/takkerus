use tak::{generation, Color, PieceType, State};

use crate::util::placement_threat_map;

use crate::ply_generator::Continuation::*;
use crate::ply_generator::Fallibility::*;
use crate::ply_generator::GeneratedPly;

use Color::*;
use PieceType::*;

pub(crate) struct PlacementWins<'a, const N: usize> {
    state: &'a State<N>,
}

impl<'a, const N: usize> PlacementWins<'a, N> {
    pub fn new(state: &'a State<N>) -> Self {
        Self { state }
    }
}

impl<'a, const N: usize> Iterator for PlacementWins<'a, N> {
    type Item = GeneratedPly<N>;

    fn next(&mut self) -> Option<Self::Item> {
        let m = &self.state.metadata;

        let all_pieces = m.p1_pieces | m.p2_pieces;
        let road_pieces = m.flatstones | m.capstones;

        let player_road_pieces = match self.state.to_move() {
            White => road_pieces & m.p1_pieces,
            Black => road_pieces & m.p2_pieces,
        };
        let blocking_pieces = all_pieces & !player_road_pieces;

        let threat_map = placement_threat_map(player_road_pieces, blocking_pieces);

        let flatstone_reserves = match self.state.to_move() {
            White => self.state.p1_flatstones,
            Black => self.state.p2_flatstones,
        };

        let piece_type = if flatstone_reserves > 0 {
            Flatstone
        } else {
            Capstone
        };

        generation::placements(threat_map, piece_type)
            .map(|ply| GeneratedPly {
                ply,
                fallibility: Infallible,
                continuation: Stop,
            })
            .next()
    }
}
