use tak::{edge_masks, generation, Bitmap, Color, Direction, PieceType, State};

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

/// Returns a map filled with all single locations that would complete a road.
fn placement_threat_map<const N: usize>(
    road_pieces: Bitmap<N>,
    blocking_pieces: Bitmap<N>,
) -> Bitmap<N> {
    use Direction::*;

    let edges = edge_masks();

    let left_pieces = edges[West as usize].flood_fill(road_pieces);
    let right_pieces = edges[East as usize].flood_fill(road_pieces);
    let horizontal_threats = (left_pieces.dilate() | edges[West as usize])
        & (right_pieces.dilate() | edges[East as usize]);

    let top_pieces = edges[North as usize].flood_fill(road_pieces);
    let bottom_pieces = edges[South as usize].flood_fill(road_pieces);
    let vertical_threats = (top_pieces.dilate() | edges[North as usize])
        & (bottom_pieces.dilate() | edges[South as usize]);

    (horizontal_threats | vertical_threats) & !blocking_pieces
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn placement_threat_maps_are_correct() {
        let b: Bitmap<5> = 0b01000_11110_01000_00000_01000.into();

        let t = placement_threat_map(b, 0.into());
        assert_eq!(t, 0b00000_00001_00000_01000_00000.into());

        let t = placement_threat_map(b, 0b01000_11111_01000_00000_01000.into());
        assert_eq!(t, 0b00000_00000_00000_01000_00000.into());

        let b: Bitmap<6> = 0b001000_111110_101010_010101_011111_000100.into();

        let t = placement_threat_map(b, 0.into());
        assert_eq!(t, 0b000000_000001_010101_101010_100000_000000.into());
    }
}
