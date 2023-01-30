use tak::{center_mask, edge_masks, Bitmap, Direction, Metadata};

use crate::util::placement_threat_map;

use super::types::EvalType;

struct Weights {
    flatstone: EvalType,
    standing_stone: EvalType,
    capstone: EvalType,
    location: EvalType,
    road_group: EvalType,
    road_anchor: EvalType,
    road_slice: EvalType,
    hard_flat: EvalType,
    soft_flat: EvalType,
    placement_threat: EvalType,
}

const WEIGHT: Weights = Weights {
    flatstone: 2000,
    standing_stone: 1000,
    capstone: 1500,
    location: 100,
    road_group: -500,
    road_anchor: -150,
    road_slice: 250,
    hard_flat: 500,
    soft_flat: -250,
    placement_threat: 1000,
};

/// Scores a player's top-most pieces by type.
pub(super) fn evaluate_material<const N: usize>(
    m: &Metadata<N>,
    player_pieces: Bitmap<N>,
) -> EvalType {
    let mut eval = 0;

    let flatstones = (player_pieces & m.flatstones).count_ones() as EvalType;
    let standing_stones = (player_pieces & m.standing_stones).count_ones() as EvalType;
    let capstones = (player_pieces & m.capstones).count_ones() as EvalType;

    eval += flatstones * WEIGHT.flatstone / N as EvalType;
    eval += standing_stones * WEIGHT.standing_stone / N as EvalType;
    eval += capstones * WEIGHT.capstone / N as EvalType;

    eval
}

/// Scores a player's blocking pieces by where they lie on the board.
pub(super) fn evaluate_blocker_locations<const N: usize>(
    player_blocking_pieces: Bitmap<N>,
) -> EvalType {
    if player_blocking_pieces == 0.into() {
        return 0;
    }
    // The number of dilations it will take to fill all edges except the corners.
    let steps = (N - 1) / 2 * 2;

    let mut eval = 0;

    let mut covered_area = center_mask();
    let mut next_area = center_mask();

    for step in 0..steps {
        let multiplier = (steps - step) as EvalType;
        let location_bonus = multiplier * WEIGHT.location / N as EvalType;

        eval += (player_blocking_pieces & next_area).count_ones() as EvalType * location_bonus;

        next_area = covered_area.dilate() ^ covered_area;
        covered_area |= next_area;
    }

    eval
}

/// Scores a player's road-contributing groups by how much of the board they
/// span in each direction, and by whether or not they touch a board edge.
pub(super) fn evaluate_road_groups<const N: usize>(player_road_pieces: Bitmap<N>) -> EvalType {
    let mut eval = 0;

    // Weight groups by what percentage of the board they cover.
    const fn size_weights<const N: usize>() -> &'static [EvalType] {
        macro_rules! w {
            ($d:literal, [$($i:literal),+]) => {{
                &[$(WEIGHT.road_group * $i / $d),+]
            }};
        }
        match N {
            3 => w!(3, [1, 2, 3]),
            4 => w!(4, [1, 2, 3, 4]),
            5 => w!(5, [1, 2, 3, 4, 5]),
            6 => w!(6, [1, 2, 3, 4, 5, 6]),
            7 => w!(7, [1, 2, 3, 4, 5, 6, 7]),
            8 => w!(8, [1, 2, 3, 4, 5, 6, 7, 8]),
            _ => unreachable!(),
        }
    }

    let horizontal_road_edges = edge_masks()[Direction::West as usize] | edge_masks()[Direction::East as usize];
    let vertical_road_edges = edge_masks()[Direction::North as usize] | edge_masks()[Direction::South as usize];

    for group in player_road_pieces.groups() {
        eval += size_weights::<N>()[group.width() - 1];
        eval += size_weights::<N>()[group.height() - 1];

        if group & horizontal_road_edges != 0.into() {
            eval += WEIGHT.road_anchor / N as EvalType;
        }

        if group & vertical_road_edges != 0.into() {
            eval += WEIGHT.road_anchor / N as EvalType;
        }
    }

    eval
}

/// Returns a bonus for each horizontal and vertical slice of the board
/// that a player has at least 1 road-contributing piece in.
pub(super) fn evaluate_road_slices<const N: usize>(player_road_pieces: Bitmap<N>) -> EvalType {
    let mut eval = 0;

    let mut row_mask = edge_masks()[Direction::North as usize];
    for _ in 0..N {
        if player_road_pieces & row_mask != 0.into() {
            eval += WEIGHT.road_slice / N as EvalType;
        }
        row_mask >>= N;
    }

    let mut column_mask = edge_masks()[Direction::West as usize];
    for _ in 0..N {
        if player_road_pieces & column_mask != 0.into() {
            eval += WEIGHT.road_slice / N as EvalType;
        }
        column_mask >>= 1;
    }

    eval
}

/// Scores a player's stacks by how many of each player's flatstones are
/// contained within them.
pub(super) fn evaluate_captured_flats<const N: usize>(
    mut player_pieces: Bitmap<N>,
    player_stacks: &[[u8; N]; N],
    opponent_stacks: &[[u8; N]; N],
) -> EvalType {
    let mut hard_flats = 0;
    let mut soft_flats = 0;

    for y in 0..N {
        for x in (0..N).rev() {
            if player_pieces & 0x01 == 1.into() {
                let player_stack = player_stacks[x][y];
                let opponent_stack = opponent_stacks[x][y];

                hard_flats += player_stack.count_ones() as u8 - 1;
                soft_flats += opponent_stack.count_ones() as u8;
            }
            player_pieces >>= 1;
        }
    }

    hard_flats as EvalType * WEIGHT.hard_flat / N as EvalType
        + soft_flats as EvalType * WEIGHT.soft_flat / N as EvalType
}

/// Returns a bonus for each empty square that would complete a road
/// if a player were to place a flatstone there.
pub(super) fn evaluate_placement_threats<const N: usize>(
    player_road_pieces: Bitmap<N>,
    blocking_pieces: Bitmap<N>,
) -> EvalType {
    let threats = placement_threat_map(player_road_pieces, blocking_pieces);

    threats.count_ones() as EvalType * WEIGHT.placement_threat / N as EvalType
}

/// Returns an offset meant to stabilize the evaluation against the even-odd effect.
pub(super) fn tempo_offset<const N: usize>(start_ply: u16, current_ply: u16) -> EvalType {
    if (current_ply - start_ply) % 2 == 1 {
        // Flip the offset at the beginning of a game since we're placing opponent pieces.
        let mul = if current_ply > 2 { 1 } else { -1 };

        // This is the value of a single, lone flatstone.
        let offset = WEIGHT.flatstone / N as EvalType
            + WEIGHT.road_group / N as EvalType * 2
            + WEIGHT.road_slice / N as EvalType * 2;

        mul * offset
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tak::State;

    #[test]
    fn material() {
        let state: State<6> = "x6/x4,2,1/x2,2,2C,1,2/x2,2,x,1,1/x5,1/x6 1 6"
            .parse()
            .unwrap();
        assert_eq!(
            evaluate_material(&state.metadata, state.metadata.p1_pieces),
            5 * WEIGHT.flatstone / 6,
        );
        assert_eq!(
            evaluate_material(&state.metadata, state.metadata.p2_pieces),
            4 * WEIGHT.flatstone / 6 + 1 * WEIGHT.capstone / 6,
        );

        let state: State<6> = "x2,21,122,1121S,112S/1S,x,1112,x,2S,x/112C,2S,x,1222221C,2,x/2,x2,1,2121S,x/112,1112111112S,x3,221S/2,2,x2,21,2 1 56".parse().unwrap();
        assert_eq!(
            evaluate_material(&state.metadata, state.metadata.p1_pieces),
            3 * WEIGHT.flatstone / 6 + 4 * WEIGHT.standing_stone / 6 + 1 * WEIGHT.capstone / 6,
        );
        assert_eq!(
            evaluate_material(&state.metadata, state.metadata.p2_pieces),
            8 * WEIGHT.flatstone / 6 + 4 * WEIGHT.standing_stone / 6 + 1 * WEIGHT.capstone / 6,
        );
    }

    #[test]
    fn blocker_locations() {
        let mut weights = [[0; 3]; 3];
        let map = Bitmap::<6>::new(0b000000_000000_000000_111000_111000_111000);

        for (b, (x, y)) in map.bits().map(|b| (b, b.coordinates())) {
            weights[x][y] = evaluate_blocker_locations(b);
        }

        assert_eq!(weights[0][1], weights[1][0]);
        assert_eq!(weights[0][2], weights[1][1]);
        assert_eq!(weights[1][1], weights[2][0]);
        assert_eq!(weights[1][2], weights[2][1]);

        assert!(weights[2][2] > weights[1][2]);
        assert!(weights[1][2] > weights[0][2]);
        assert!(weights[0][2] > weights[0][1]);
        assert!(weights[0][1] > weights[0][0]);
    }
}
