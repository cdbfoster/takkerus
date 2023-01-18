pub use self::util::Evaluation;

pub(crate) use self::util::placement_threat_maps;

use self::util::EvalType;

mod util;

use std::fmt;

use tak::{edge_masks, Bitmap, Color, Direction, Metadata, Resolution, State};

pub fn evaluate<const N: usize>(state: &State<N>) -> Evaluation {
    use Color::*;

    let to_move = if state.ply_count % 2 == 0 {
        Color::White
    } else {
        Color::Black
    };

    match state.resolution() {
        None => (),
        Some(Resolution::Road(color)) | Some(Resolution::Flats { color, .. }) => {
            if color == to_move {
                return Evaluation::WIN - state.ply_count as i32;
            } else {
                return Evaluation::LOSE + state.ply_count as i32;
            }
        }
        Some(Resolution::Draw) => return Evaluation::ZERO - state.ply_count as i32,
    }

    let m = &state.metadata;

    let mut p1_eval = Evaluation::ZERO;
    let mut p2_eval = Evaluation::ZERO;

    let road_pieces = m.flatstones | m.capstones;
    let p1_road_pieces = road_pieces & m.p1_pieces;
    let p2_road_pieces = road_pieces & m.p2_pieces;
    let all_pieces = m.p1_pieces & m.p2_pieces;

    // Material
    p1_eval += evaluate_material(m, m.p1_pieces);
    p2_eval += evaluate_material(m, m.p2_pieces);

    // Road groups
    p1_eval += evaluate_road_groups(p1_road_pieces);
    p2_eval += evaluate_road_groups(p2_road_pieces);

    // Road slices
    p1_eval += evaluate_road_slices(p1_road_pieces);
    p2_eval += evaluate_road_slices(p2_road_pieces);

    // Captured flats
    p1_eval += evaluate_captured_flats(m.p1_pieces, &m.p1_stacks, &m.p2_stacks);
    p2_eval += evaluate_captured_flats(m.p2_pieces, &m.p2_stacks, &m.p1_stacks);

    // Placement threats
    p1_eval += evaluate_placement_threats(all_pieces, p1_road_pieces);
    p2_eval += evaluate_placement_threats(all_pieces, p2_road_pieces);

    match to_move {
        White => p1_eval - p2_eval,
        Black => p2_eval - p1_eval,
    }
}

struct Weights {
    flatstone: EvalType,
    standing_stone: EvalType,
    capstone: EvalType,
    road_group: EvalType,
    road_slice: EvalType,
    hard_flat: EvalType,
    soft_flat: EvalType,
    placement_threat: EvalType,
}

const WEIGHT: Weights = Weights {
    flatstone: 2000,
    standing_stone: 1000,
    capstone: 1500,
    road_group: -500,
    road_slice: 250,
    hard_flat: 500,
    soft_flat: -250,
    placement_threat: 1000,
};

fn evaluate_material<const N: usize>(m: &Metadata<N>, pieces: Bitmap<N>) -> EvalType {
    let mut eval = 0;
    eval += (pieces & m.flatstones).count_ones() as EvalType * WEIGHT.flatstone / N as EvalType;
    eval += (pieces & m.standing_stones).count_ones() as EvalType * WEIGHT.standing_stone
        / N as EvalType;
    eval += (pieces & m.capstones).count_ones() as EvalType * WEIGHT.capstone / N as EvalType;
    eval
}

fn evaluate_road_groups<const N: usize>(road_pieces: Bitmap<N>) -> EvalType {
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

    for group in road_pieces.groups() {
        eval += size_weights::<N>()[group.width() - 1];
        eval += size_weights::<N>()[group.height() - 1];
    }

    eval
}

fn evaluate_road_slices<const N: usize>(road_pieces: Bitmap<N>) -> EvalType {
    let mut eval = 0;

    let mut row_mask = edge_masks::<N>()[Direction::North as usize];
    for _ in 0..N {
        if road_pieces & row_mask != 0.into() {
            eval += WEIGHT.road_slice / N as EvalType;
        }
        row_mask >>= N;
    }

    let mut column_mask = edge_masks::<N>()[Direction::West as usize];
    for _ in 0..N {
        if road_pieces & column_mask != 0.into() {
            eval += WEIGHT.road_slice / N as EvalType;
        }
        column_mask >>= 1;
    }

    eval
}

fn evaluate_captured_flats<const N: usize>(
    mut pieces: Bitmap<N>,
    player_stacks: &[[u8; N]; N],
    opponent_stacks: &[[u8; N]; N],
) -> EvalType {
    let mut hard_flats = 0;
    let mut soft_flats = 0;

    for y in 0..N {
        for x in (0..N).rev() {
            if pieces & 0x01 == 1.into() {
                let player_stack = player_stacks[x][y];
                let opponent_stack = opponent_stacks[x][y];

                hard_flats += player_stack.count_ones() as u8 - 1;
                soft_flats += opponent_stack.count_ones() as u8;
            }
            pieces >>= 1;
        }
    }

    hard_flats as EvalType * WEIGHT.hard_flat / N as EvalType
        + soft_flats as EvalType * WEIGHT.soft_flat / N as EvalType
}

fn evaluate_placement_threats<const N: usize>(
    all_pieces: Bitmap<N>,
    road_pieces: Bitmap<N>,
) -> EvalType {
    let (horizontal_threats, vertical_threats) = placement_threat_maps(all_pieces, road_pieces);

    let mut eval = 0;
    eval += horizontal_threats.count_ones() as EvalType * WEIGHT.placement_threat / N as EvalType;
    eval += vertical_threats.count_ones() as EvalType * WEIGHT.placement_threat / N as EvalType;

    eval
}

impl From<EvalType> for Evaluation {
    fn from(value: EvalType) -> Self {
        Self(value)
    }
}

impl fmt::Display for Evaluation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
