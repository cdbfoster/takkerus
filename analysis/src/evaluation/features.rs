#![allow(clippy::unusual_byte_groupings)]

use ann::linear_algebra::Vector;
use tak::{edge_masks, Bitmap, Color, Direction, State};

use crate::util::placement_threat_map;

use Color::*;
use Direction::*;

pub trait GatherFeatures {
    const FEATURES: usize;
    type Features;

    fn gather_features(&self) -> Self::Features;
}

macro_rules! features_impl {
    (size: $size:expr, module: $module:ident, symmetries: $sym:expr, maps: $maps:expr) => {
        pub mod $module {
            use super::*;

            const POSITIONS: usize = $sym;

            const POSITION_MAPS: [Bitmap<$size>; POSITIONS] = $maps;

            // Adjacency table:
            //
            // P = Player
            // O = Opponent
            // F = Flatstone
            // S = Standing stone
            // C = Capstone
            //
            //     PF  PS  PC  OF  OS  OC
            // PF
            // PS
            // PC

            /// A table to track the number of pieces adjacent to others.
            const ADJACENCY_FEATURES: usize = 18;

            const FEATURES: usize = 1   // Ply count / 10
                + 1                     // White to move
                + 1                     // FCD (with komi)
                + 2                     // Reserves (flatstones and capstones)
                + POSITIONS             // Flatstones in each position
                + POSITIONS             // Standing stones in each position
                + POSITIONS             // Capstones in each position
                + ADJACENCY_FEATURES    // Piece type adjacency table
                + 3                     // Captives under each piece type
                + 3                     // Friendlies under each piece type
                + 1                     // Lines occupied (road pieces)
                + 1                     // Road groups
                + 1                     // Critical squares
                ;

            #[repr(C)]
            #[derive(Default, Debug)]
            pub struct Features {
                pub ply_count: f32,
                pub white_to_move: f32,
                pub fcd: f32,
                pub reserve_flatstones: f32,
                pub reserve_capstones: f32,
                pub flatstone_positions: [f32; POSITIONS],
                pub standing_stone_positions: [f32; POSITIONS],
                pub capstone_positions: [f32; POSITIONS],
                pub piece_adjacency: [[f32; 6]; 3],
                pub captives: [f32; 3],
                pub friendlies: [f32; 3],
                pub lines_occupied: f32,
                pub road_groups: f32,
                pub critical_squares: f32,
            }

            impl Features {
                pub fn as_vector(&self) -> &Vector<FEATURES> {
                    unsafe { std::mem::transmute(self) }
                }
            }

            impl GatherFeatures for State<$size> {
                const FEATURES: usize = FEATURES;
                type Features = Features;

                fn gather_features(&self) -> Self::Features {
                    let mut p1_features = Self::Features::default();
                    let mut p2_features = Self::Features::default();

                    let m = &self.metadata;

                    let all_pieces = m.p1_pieces & m.p2_pieces;

                    let p1_flatstones = m.flatstones & m.p1_pieces;
                    let p2_flatstones = m.flatstones & m.p2_pieces;
                    let p1_standing_stones = m.standing_stones & m.p1_pieces;
                    let p2_standing_stones = m.standing_stones & m.p2_pieces;
                    let p1_capstones = m.capstones & m.p1_pieces;
                    let p2_capstones = m.capstones & m.p2_pieces;

                    let p1_road_pieces = p1_flatstones | p1_capstones;
                    let p2_road_pieces = p2_flatstones | p2_capstones;

                    p1_features.fcd = gather_fcd(p1_flatstones, 0.0);
                    p2_features.fcd = gather_fcd(p2_flatstones, self.komi.as_f32());

                    p1_features.reserve_flatstones = self.p1_flatstones as f32;
                    p2_features.reserve_flatstones = self.p2_flatstones as f32;

                    p1_features.reserve_capstones = self.p1_capstones as f32;
                    p2_features.reserve_capstones = self.p2_capstones as f32;

                    p1_features.flatstone_positions = gather_positions(p1_flatstones, POSITION_MAPS);
                    p2_features.flatstone_positions = gather_positions(p2_flatstones, POSITION_MAPS);

                    p1_features.standing_stone_positions =
                        gather_positions(p1_standing_stones, POSITION_MAPS);
                    p2_features.standing_stone_positions =
                        gather_positions(p2_standing_stones, POSITION_MAPS);

                    p1_features.capstone_positions = gather_positions(p1_capstones, POSITION_MAPS);
                    p2_features.capstone_positions = gather_positions(p2_capstones, POSITION_MAPS);

                    p1_features.piece_adjacency = gather_piece_adjacency([
                        p1_flatstones,
                        p1_standing_stones,
                        p1_capstones,
                        p2_flatstones,
                        p2_standing_stones,
                        p2_capstones,
                    ]);
                    p2_features.piece_adjacency = gather_piece_adjacency([
                        p2_flatstones,
                        p2_standing_stones,
                        p2_capstones,
                        p1_flatstones,
                        p1_standing_stones,
                        p1_capstones,
                    ]);

                    p1_features.captives = gather_captives(
                        [p1_flatstones, p1_standing_stones, p1_capstones],
                        &m.p2_stacks,
                    );
                    p2_features.captives = gather_captives(
                        [p2_flatstones, p2_standing_stones, p2_capstones],
                        &m.p1_stacks,
                    );

                    p1_features.friendlies = gather_friendlies(
                        [p1_flatstones, p1_standing_stones, p1_capstones],
                        &m.p1_stacks,
                    );
                    p2_features.friendlies = gather_friendlies(
                        [p2_flatstones, p2_standing_stones, p2_capstones],
                        &m.p2_stacks,
                    );

                    p1_features.lines_occupied = gather_lines_occupied(p1_road_pieces);
                    p2_features.lines_occupied = gather_lines_occupied(p2_road_pieces);

                    p1_features.road_groups = (p1_road_pieces).groups().count() as f32;
                    p2_features.road_groups = (p2_road_pieces).groups().count() as f32;

                    p1_features.critical_squares =
                        gather_critical_squares(p1_road_pieces, all_pieces & !p1_road_pieces);
                    p2_features.critical_squares =
                        gather_critical_squares(p2_road_pieces, all_pieces & !p2_road_pieces);

                    let features_vector = match self.to_move() {
                        White => p1_features.as_vector() - p2_features.as_vector(),
                        Black => p2_features.as_vector() - p1_features.as_vector(),
                    };

                    let mut features: Self::Features = unsafe { std::mem::transmute(features_vector) };

                    features.ply_count = self.ply_count as f32 / 10.0;
                    features.white_to_move = if self.to_move() == White { 1.0 } else { 0.0 };

                    features
                }
            }
        }
    };
}

features_impl!(
    size: 3,
    module: features_3s,
    symmetries: 3,
    maps: [
        Bitmap::new(0b101_000_101),
        Bitmap::new(0b010_101_010),
        Bitmap::new(0b000_010_000),
    ]
);

features_impl!(
    size: 4,
    module: features_4s,
    symmetries: 3,
    maps: [
        Bitmap::new(0b1001_0000_0000_1001),
        Bitmap::new(0b0110_1001_1001_0110),
        Bitmap::new(0b0000_0110_0110_0000),
    ]
);

features_impl!(
    size: 5,
    module: features_5s,
    symmetries: 6,
    maps: [
        Bitmap::new(0b10001_00000_00000_00000_10001),
        Bitmap::new(0b01010_10001_00000_10001_01010),
        Bitmap::new(0b00100_00000_10001_00000_00100),
        Bitmap::new(0b00000_01010_00000_01010_00000),
        Bitmap::new(0b00000_00100_01010_00100_00000),
        Bitmap::new(0b00000_00000_00100_00000_00000),
    ]
);

features_impl!(
    size: 6,
    module: features_6s,
    symmetries: 6,
    maps: [
        Bitmap::new(0b100001_000000_000000_000000_000000_100001),
        Bitmap::new(0b010010_100001_000000_000000_100001_010010),
        Bitmap::new(0b001100_000000_100001_100001_000000_001100),
        Bitmap::new(0b000000_010010_000000_000000_010010_000000),
        Bitmap::new(0b000000_001100_010010_010010_001100_000000),
        Bitmap::new(0b000000_000000_001100_001100_000000_000000),
    ]
);

features_impl!(
    size: 7,
    module: features_7s,
    symmetries: 10,
    maps: [
        Bitmap::new(0b1000001_0000000_0000000_0000000_0000000_0000000_1000001),
        Bitmap::new(0b0100010_1000001_0000000_0000000_0000000_1000001_0100010),
        Bitmap::new(0b0010100_0000000_1000001_0000000_1000001_0000000_0010100),
        Bitmap::new(0b0001000_0000000_0000000_1000001_0000000_0000000_0001000),
        Bitmap::new(0b0000000_0100010_0000000_0000000_0000000_0100010_0000000),
        Bitmap::new(0b0000000_0010100_0100010_0000000_0100010_0010100_0000000),
        Bitmap::new(0b0000000_0001000_0000000_0100010_0000000_0001000_0000000),
        Bitmap::new(0b0000000_0000000_0010100_0000000_0010100_0000000_0000000),
        Bitmap::new(0b0000000_0000000_0001000_0010100_0001000_0000000_0000000),
        Bitmap::new(0b0000000_0000000_0000000_0001000_0000000_0000000_0000000),
    ]
);

features_impl!(
    size: 8,
    module: features_8s,
    symmetries: 10,
    maps: [
        Bitmap::new(0b10000001_00000000_00000000_00000000_00000000_00000000_00000000_10000001),
        Bitmap::new(0b01000010_10000001_00000000_00000000_00000000_00000000_10000001_01000010),
        Bitmap::new(0b00100100_00000000_10000001_00000000_00000000_10000001_00000000_00100100),
        Bitmap::new(0b00011000_00000000_00000000_10000001_10000001_00000000_00000000_00011000),
        Bitmap::new(0b00000000_01000010_00000000_00000000_00000000_00000000_01000010_00000000),
        Bitmap::new(0b00000000_00100100_01000010_00000000_00000000_01000010_00100100_00000000),
        Bitmap::new(0b00000000_00011000_00000000_01000010_01000010_00000000_00011000_00000000),
        Bitmap::new(0b00000000_00000000_00100100_00000000_00000000_00100100_00000000_00000000),
        Bitmap::new(0b00000000_00000000_00011000_00100100_00100100_00011000_00000000_00000000),
        Bitmap::new(0b00000000_00000000_00000000_00011000_00011000_00000000_00000000_00000000),
    ]
);

fn gather_fcd<const N: usize>(player_flatstones: Bitmap<N>, komi: f32) -> f32 {
    player_flatstones.count_ones() as f32 + komi
}

fn gather_positions<const N: usize, const P: usize>(
    player_pieces: Bitmap<N>,
    maps: [Bitmap<N>; P],
) -> [f32; P] {
    maps.map(|map| (player_pieces & map).count_ones() as f32)
}

fn gather_piece_adjacency<const N: usize>(all_pieces: [Bitmap<N>; 6]) -> [[f32; 6]; 3] {
    let mut counts = [[0.0; 6]; 3];

    for piece_type in 0..3 {
        for other_piece in 0..6 {
            if piece_type == other_piece {
                counts[piece_type][piece_type] = all_pieces[piece_type]
                    .groups()
                    .map(|g| g.count_ones())
                    .filter(|&c| c > 1)
                    .sum::<u32>() as f32;
            } else {
                counts[piece_type][other_piece] =
                    (all_pieces[other_piece].dilate() & all_pieces[piece_type]).count_ones() as f32;
            }
        }
    }

    counts
}

fn gather_captives<const N: usize>(
    player_pieces: [Bitmap<N>; 3],
    opponent_stacks: &[[u8; N]; N],
) -> [f32; 3] {
    player_pieces.map(|pieces| {
        pieces
            .bits()
            .map(|b| b.coordinates())
            .map(|(x, y)| opponent_stacks[x][y].count_ones())
            .sum::<u32>() as f32
    })
}

fn gather_friendlies<const N: usize>(
    player_pieces: [Bitmap<N>; 3],
    player_stacks: &[[u8; N]; N],
) -> [f32; 3] {
    player_pieces.map(|pieces| {
        pieces
            .bits()
            .map(|b| b.coordinates())
            .map(|(x, y)| player_stacks[x][y].count_ones() - 1)
            .sum::<u32>() as f32
    })
}

fn gather_lines_occupied<const N: usize>(player_pieces: Bitmap<N>) -> f32 {
    let mut mask = edge_masks()[West as usize];
    let mut rows = Bitmap::empty();

    let mut lines = 0;

    for i in 0..N {
        let column = player_pieces & mask;

        if !column.is_empty() {
            lines += 1; // Count this vertical line as occupied.
            rows |= column << i;
        }

        mask >>= i;
    }

    lines += rows.count_ones(); // Count the horizontal lines occupied.
    lines as f32
}

fn gather_critical_squares<const N: usize>(
    player_road_pieces: Bitmap<N>,
    blocking_pieces: Bitmap<N>,
) -> f32 {
    placement_threat_map(player_road_pieces, blocking_pieces).count_ones() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_feature_counts() {
        for n in 3..=8 {
            let count = match n {
                3 => <State<3> as GatherFeatures>::FEATURES,
                4 => <State<4> as GatherFeatures>::FEATURES,
                5 => <State<5> as GatherFeatures>::FEATURES,
                6 => <State<6> as GatherFeatures>::FEATURES,
                7 => <State<7> as GatherFeatures>::FEATURES,
                8 => <State<8> as GatherFeatures>::FEATURES,
                _ => unreachable!(),
            };

            println!("State<{n}>: {count} features");
        }
    }
}
