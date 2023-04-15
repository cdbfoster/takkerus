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

            const FEATURES: usize = 1   // White to move
                + 1                     // Flat count differential
                + 4                     // Reserves (flatsones and capstones)
                + 2 * 3                 // Friendlies under each piece type
                + 2 * 3                 // Captives under each piece type
                + 2 * POSITIONS         // Flatstones in each position
                + 2 * POSITIONS         // Capstones in each position
                + 2                     // Road groups
                + 2                     // Lines occupied
                + 2                     // Critical squares
                + 2                     // Enemy flatstones next to our standing stones
                + 2                     // Enemy flatstones next to our capstones
                ;

            #[repr(C)]
            #[derive(Debug, Default, PartialEq)]
            pub struct PlayerFeatures {
                pub reserve_flatstones: f32,
                pub reserve_capstones: f32,
                pub friendlies: [f32; 3],
                pub captives: [f32; 3],
                pub flatstone_positions: [f32; POSITIONS],
                pub capstone_positions: [f32; POSITIONS],
                pub road_groups: f32,
                pub lines_occupied: f32,
                pub critical_squares: f32,
                pub standing_stone_surroundings: f32,
                pub capstone_surroundings: f32,
            }

            #[repr(C)]
            #[derive(Debug, Default, PartialEq)]
            pub struct Features {
                pub white_to_move: f32,
                pub fcd: f32,
                pub player: PlayerFeatures,
                pub opponent: PlayerFeatures,
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
                    let mut p1 = PlayerFeatures::default();
                    let mut p2 = PlayerFeatures::default();

                    let m = &self.metadata;

                    let all_pieces = m.p1_pieces | m.p2_pieces;

                    let p1_flatstones = m.flatstones & m.p1_pieces;
                    let p2_flatstones = m.flatstones & m.p2_pieces;
                    let p1_standing_stones = m.standing_stones & m.p1_pieces;
                    let p2_standing_stones = m.standing_stones & m.p2_pieces;
                    let p1_capstones = m.capstones & m.p1_pieces;
                    let p2_capstones = m.capstones & m.p2_pieces;

                    let p1_road_pieces = p1_flatstones | p1_capstones;
                    let p2_road_pieces = p2_flatstones | p2_capstones;

                    let p1_flat_count = gather_flat_count(p1_flatstones, 0.0);
                    let p2_flat_count = gather_flat_count(p2_flatstones, self.komi.as_f32());

                    let (starting_flatstones, starting_capstones) = Self::reserves();

                    p1.reserve_flatstones = self.p1_flatstones as f32 / starting_flatstones as f32;
                    p2.reserve_flatstones = self.p2_flatstones as f32 / starting_flatstones as f32;

                    p1.reserve_capstones = self.p1_capstones as f32 / starting_capstones as f32;
                    p2.reserve_capstones = self.p2_capstones as f32 / starting_capstones as f32;

                    let (friendlies, captives) = gather_stack_composition([p1_flatstones, p1_standing_stones, p1_capstones], self, White);
                    p1.friendlies = friendlies;
                    p1.captives = captives;

                    let (friendlies, captives) = gather_stack_composition([p2_flatstones, p2_standing_stones, p2_capstones], self, Black);
                    p2.friendlies = friendlies;
                    p2.captives = captives;

                    p1.flatstone_positions = gather_positions(p1_flatstones, POSITION_MAPS);
                    p2.flatstone_positions = gather_positions(p2_flatstones, POSITION_MAPS);

                    p1.capstone_positions = gather_positions(p1_capstones, POSITION_MAPS);
                    p2.capstone_positions = gather_positions(p2_capstones, POSITION_MAPS);

                    p1.road_groups = (p1_road_pieces).groups().count() as f32;
                    p2.road_groups = (p2_road_pieces).groups().count() as f32;

                    p1.lines_occupied = gather_lines_occupied(p1_road_pieces);
                    p2.lines_occupied = gather_lines_occupied(p2_road_pieces);

                    p1.critical_squares =
                        gather_critical_squares(p1_road_pieces, all_pieces & !p1_road_pieces);
                    p2.critical_squares =
                        gather_critical_squares(p2_road_pieces, all_pieces & !p2_road_pieces);

                    p1.standing_stone_surroundings = (p1_standing_stones.dilate() & p2_flatstones).count_ones() as f32;
                    p2.standing_stone_surroundings = (p2_standing_stones.dilate() & p1_flatstones).count_ones() as f32;

                    p1.capstone_surroundings = (p1_capstones.dilate() & p2_flatstones).count_ones() as f32;
                    p2.capstone_surroundings = (p2_capstones.dilate() & p1_flatstones).count_ones() as f32;

                    match self.to_move() {
                        White => Features {
                            white_to_move: 1.0,
                            fcd: p1_flat_count - p2_flat_count,
                            player: p1,
                            opponent: p2,
                        },
                        Black => Features {
                            white_to_move: 0.0,
                            fcd: p2_flat_count - p1_flat_count,
                            player: p2,
                            opponent: p1,
                        },
                    }
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
        Bitmap::new(0b000_010_000),
        Bitmap::new(0b010_101_010),
        Bitmap::new(0b101_000_101),
    ]
);

features_impl!(
    size: 4,
    module: features_4s,
    symmetries: 3,
    maps: [
        Bitmap::new(0b0000_0110_0110_0000),
        Bitmap::new(0b0110_1001_1001_0110),
        Bitmap::new(0b1001_0000_0000_1001),
    ]
);

features_impl!(
    size: 5,
    module: features_5s,
    symmetries: 5,
    maps: [
        Bitmap::new(0b00000_00000_00100_00000_00000),
        Bitmap::new(0b00000_00100_01010_00100_00000),
        Bitmap::new(0b00100_01010_10001_01010_00100),
        Bitmap::new(0b01010_10001_00000_10001_01010),
        Bitmap::new(0b10001_00000_00000_00000_10001),
    ]
);

features_impl!(
    size: 6,
    module: features_6s,
    symmetries: 5,
    maps: [
        Bitmap::new(0b000000_000000_001100_001100_000000_000000),
        Bitmap::new(0b000000_001100_010010_010010_001100_000000),
        Bitmap::new(0b001100_010010_100001_100001_010010_001100),
        Bitmap::new(0b010010_100001_000000_000000_100001_010010),
        Bitmap::new(0b100001_000000_000000_000000_000000_100001),
    ]
);

features_impl!(
    size: 7,
    module: features_7s,
    symmetries: 7,
    maps: [
        Bitmap::new(0b0000000_0000000_0000000_0001000_0000000_0000000_0000000),
        Bitmap::new(0b0000000_0000000_0001000_0010100_0001000_0000000_0000000),
        Bitmap::new(0b0000000_0001000_0010100_0100010_0010100_0001000_0000000),
        Bitmap::new(0b0001000_0010100_0100010_1000001_0100010_0010100_0001000),
        Bitmap::new(0b0010100_0100010_1000001_0000000_1000001_0100010_0010100),
        Bitmap::new(0b0100010_1000001_0000000_0000000_0000000_1000001_0100010),
        Bitmap::new(0b1000001_0000000_0000000_0000000_0000000_0000000_1000001),
    ]
);

features_impl!(
    size: 8,
    module: features_8s,
    symmetries: 7,
    maps: [
        Bitmap::new(0b00000000_00000000_00000000_00011000_00011000_00000000_00000000_00000000),
        Bitmap::new(0b00000000_00000000_00011000_00100100_00100100_00011000_00000000_00000000),
        Bitmap::new(0b00000000_00011000_00100100_01000010_01000010_00100100_00011000_00000000),
        Bitmap::new(0b00011000_00100100_01000010_10000001_10000001_01000010_00100100_00011000),
        Bitmap::new(0b00100100_01000010_10000001_00000000_00000000_10000001_01000010_00100100),
        Bitmap::new(0b01000010_10000001_00000000_00000000_00000000_00000000_10000001_01000010),
        Bitmap::new(0b10000001_00000000_00000000_00000000_00000000_00000000_00000000_10000001),
    ]
);

fn gather_flat_count<const N: usize>(player_flatstones: Bitmap<N>, komi: f32) -> f32 {
    player_flatstones.count_ones() as f32 + komi
}

fn gather_positions<const N: usize, const P: usize>(
    player_pieces: Bitmap<N>,
    maps: [Bitmap<N>; P],
) -> [f32; P] {
    maps.map(|map| (player_pieces & map).count_ones() as f32)
}

fn gather_stack_composition<const N: usize>(
    player_pieces: [Bitmap<N>; 3],
    state: &State<N>,
    color: Color,
) -> ([f32; 3], [f32; 3]) {
    let mut white = [0.0; 3];
    let mut black = [0.0; 3];

    for (pieces, (w_sum, b_sum)) in player_pieces
        .into_iter()
        .zip(white.iter_mut().zip(black.iter_mut()))
    {
        for (w, b) in pieces
            .bits()
            .map(|b| b.coordinates())
            .map(|(x, y)| state.board[x][y].get_player_pieces())
        {
            *w_sum += w.count_ones() as f32;
            *b_sum += b.count_ones() as f32;
        }

        match color {
            White => *w_sum -= pieces.count_ones() as f32,
            Black => *b_sum -= pieces.count_ones() as f32,
        }
    }

    match color {
        White => (white, black),
        Black => (black, white),
    }
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

        mask >>= 1;
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

    #[test]
    fn correct_features() {
        let state: State<6> = "2,1221122,1,1,1,2S/1,1,1,x,1C,1111212/x2,2,212,2C,11/2,2,x2,1,1/x3,1,1,x/x2,2,21,x,112S 1 32".parse().unwrap();
        let f = state.gather_features();
        let c = features_6s::Features {
            white_to_move: 1.0,
            fcd: 4.0,
            player: features_6s::PlayerFeatures {
                reserve_flatstones: 6.0 / 30.0,
                reserve_capstones: 0.0,
                friendlies: [1.0, 0.0, 0.0],
                captives: [1.0, 0.0, 0.0],
                flatstone_positions: [0.0, 3.0, 7.0, 2.0, 0.0],
                capstone_positions: [0.0, 0.0, 1.0, 0.0, 0.0],
                road_groups: 2.0,
                lines_occupied: 12.0,
                critical_squares: 0.0,
                standing_stone_surroundings: 0.0,
                capstone_surroundings: 1.0,
            },
            opponent: features_6s::PlayerFeatures {
                reserve_flatstones: 14.0 / 30.0,
                reserve_capstones: 0.0,
                friendlies: [5.0, 0.0, 0.0],
                captives: [9.0, 2.0, 0.0],
                flatstone_positions: [2.0, 1.0, 2.0, 2.0, 1.0],
                capstone_positions: [0.0, 1.0, 0.0, 0.0, 0.0],
                road_groups: 5.0,
                lines_occupied: 11.0,
                critical_squares: 0.0,
                standing_stone_surroundings: 1.0,
                capstone_surroundings: 2.0,
            },
        };
        assert_eq!(f, c);

        let state: State<7> = "2,2,21S,2,1,1,1/2,1,x,2,1,x,1/2,2,2,2,21112C,121S,x/x2,1112C,2,1,1112S,x/121,22211C,1S,1,1,121,1221C/x,2,2,2,1,12,2/2,x3,1,122,x 2 50".parse().unwrap();
        let f = state.gather_features();
        let c = features_7s::Features {
            white_to_move: 0.0,
            fcd: 4.0,
            player: features_7s::PlayerFeatures {
                reserve_flatstones: 11.0 / 40.0,
                reserve_capstones: 0.0,
                friendlies: [1.0, 0.0, 1.0],
                captives: [2.0, 3.0, 6.0],
                flatstone_positions: [1.0, 1.0, 3.0, 3.0, 3.0, 4.0, 2.0],
                capstone_positions: [0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                road_groups: 4.0,
                lines_occupied: 13.0,
                critical_squares: 0.0,
                standing_stone_surroundings: 2.0,
                capstone_surroundings: 2.0,
            },
            opponent: features_7s::PlayerFeatures {
                reserve_flatstones: 8.0 / 40.0,
                reserve_capstones: 0.0,
                friendlies: [2.0, 1.0, 2.0],
                captives: [2.0, 2.0, 5.0],
                flatstone_positions: [0.0, 2.0, 1.0, 3.0, 4.0, 2.0, 1.0],
                capstone_positions: [0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0],
                road_groups: 4.0,
                lines_occupied: 12.0,
                critical_squares: 0.0,
                standing_stone_surroundings: 3.0,
                capstone_surroundings: 2.0,
            },
        };
        assert_eq!(f, c);
    }
}
