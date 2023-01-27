use tak::{edge_masks, Bitmap, Direction};

/// Returns a map filled with all single locations that would complete a road.
pub(crate) fn placement_threat_map<const N: usize>(
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
mod tests {
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
