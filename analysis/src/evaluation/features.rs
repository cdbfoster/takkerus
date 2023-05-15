#![allow(clippy::unusual_byte_groupings)]

use ann::linear_algebra::Vector;
use tak::{edge_masks, Bitmap, Color, Direction, State};

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

            const FEATURES: usize = 1   // Flat count differential
                + 2                     // Flatstone reserves
                + 2 * 3                 // Shallow friendlies under each piece type
                + 2 * 3                 // Shallow captives under each piece type
                + 2 * 3                 // Deep friendlies under each piece type
                + 2 * 3                 // Deep captives under each piece type
                + 2 * POSITIONS         // Flatstones in each position
                + 2 * POSITIONS         // Capstones in each position
                + 2                     // Road groups
                + 2                     // Lines occupied
                + 2                     // Enemy flatstones next to our standing stones
                + 2                     // Enemy flatstones next to our capstones
                + 2                     // Unblocked road completion
                + 2                     // Soft-blocked road completion
                ;

            #[repr(C)]
            #[derive(Debug, Default, PartialEq)]
            pub struct PlayerFeatures {
                pub reserve_flatstones: f32,
                pub stack_composition: StackComposition,
                pub flatstone_positions: [f32; POSITIONS],
                pub capstone_positions: [f32; POSITIONS],
                pub road_groups: f32,
                pub lines_occupied: f32,
                pub standing_stone_surroundings: f32,
                pub capstone_surroundings: f32,
                pub unblocked_road_completion: f32,
                pub softblocked_road_completion: f32,
            }

            #[repr(C)]
            #[derive(Debug, Default, PartialEq)]
            pub struct Features {
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

                    let (starting_flatstones, _starting_capstones) = Self::reserves();

                    p1.reserve_flatstones = self.p1_flatstones as f32 / starting_flatstones as f32;
                    p2.reserve_flatstones = self.p2_flatstones as f32 / starting_flatstones as f32;

                    p1.stack_composition = gather_stack_composition([p1_flatstones, p1_standing_stones, p1_capstones], self, White);
                    p2.stack_composition = gather_stack_composition([p2_flatstones, p2_standing_stones, p2_capstones], self, Black);

                    p1.flatstone_positions = gather_positions(p1_flatstones, POSITION_MAPS);
                    p2.flatstone_positions = gather_positions(p2_flatstones, POSITION_MAPS);

                    p1.capstone_positions = gather_positions(p1_capstones, POSITION_MAPS);
                    p2.capstone_positions = gather_positions(p2_capstones, POSITION_MAPS);

                    p1.road_groups = (p1_road_pieces).groups().count() as f32;
                    p2.road_groups = (p2_road_pieces).groups().count() as f32;

                    p1.lines_occupied = gather_lines_occupied(p1_road_pieces);
                    p2.lines_occupied = gather_lines_occupied(p2_road_pieces);

                    p1.standing_stone_surroundings = (p1_standing_stones.dilate() & p2_flatstones).count_ones() as f32;
                    p2.standing_stone_surroundings = (p2_standing_stones.dilate() & p1_flatstones).count_ones() as f32;

                    p1.capstone_surroundings = (p1_capstones.dilate() & p2_flatstones).count_ones() as f32;
                    p2.capstone_surroundings = (p2_capstones.dilate() & p1_flatstones).count_ones() as f32;

                    p1.unblocked_road_completion = gather_road_steps(p1_road_pieces, all_pieces & !p1_road_pieces);
                    p2.unblocked_road_completion = gather_road_steps(p2_road_pieces, all_pieces & !p2_road_pieces);

                    p1.softblocked_road_completion = gather_road_steps(p1_road_pieces, p1_standing_stones | p2_standing_stones | p2_capstones);
                    p2.softblocked_road_completion = gather_road_steps(p2_road_pieces, p2_standing_stones | p1_standing_stones | p1_capstones);

                    match self.to_move() {
                        White => Features {
                            fcd: p1_flat_count - p2_flat_count,
                            player: p1,
                            opponent: p2,
                        },
                        Black => Features {
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

fn gather_flat_count<const N: usize>(player_flatstones: Bitmap<N>, komi: f32) -> f32 {
    player_flatstones.count_ones() as f32 + komi
}

fn gather_positions<const N: usize, const P: usize>(
    player_pieces: Bitmap<N>,
    maps: [Bitmap<N>; P],
) -> [f32; P] {
    maps.map(|map| (player_pieces & map).count_ones() as f32)
}

#[repr(C)]
#[derive(Debug, Default, PartialEq)]
pub struct StackComposition {
    pub shallow_friendlies: [f32; 3],
    pub shallow_captives: [f32; 3],
    pub deep_friendlies: [f32; 3],
    pub deep_captives: [f32; 3],
}

fn gather_stack_composition<const N: usize>(
    player_pieces: [Bitmap<N>; 3],
    state: &State<N>,
    color: Color,
) -> StackComposition {
    let mut white = StackComposition::default();

    for (i, pieces) in player_pieces
        .into_iter()
        .enumerate()
    {
        for (w, b) in pieces
            .bits()
            .map(|b| b.coordinates())
            .map(|(x, y)| state.board[x][y].get_player_pieces())
        {
            #[cfg(not(feature = "deep-stacks"))]
            type StackBitmap = u32;

            #[cfg(feature = "deep-stacks")]
            type StackBitmap = u128;

            let shallow = !(StackBitmap::MAX << N);
            let deep = StackBitmap::MAX & !shallow;

            white.shallow_friendlies[i] += (w & shallow).count_ones() as f32;
            white.shallow_captives[i] += (b & shallow).count_ones() as f32;
            white.deep_friendlies[i] += (w & deep).count_ones() as f32;
            white.deep_captives[i] += (b & deep).count_ones() as f32;
        }

        match color {
            White => white.shallow_friendlies[i] -= pieces.count_ones() as f32,
            Black => white.shallow_captives[i] -= pieces.count_ones() as f32,
        }
    }

    match color {
        White => white,
        Black => {
            StackComposition {
                shallow_friendlies: white.shallow_captives,
                shallow_captives: white.shallow_friendlies,
                deep_friendlies: white.deep_captives,
                deep_captives: white.deep_friendlies,
            }
        },
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

const UNREACHABLE: usize = 100;

/// Calculates the number of road contributing pieces that are required to complete
/// a road. Returns `0` if there's already a road, and [UNREACHABLE] if a road is impossible.
fn calculate_road_steps<const N: usize>(
    player_road_pieces: Bitmap<N>,
    blocking_pieces: Bitmap<N>,
) -> usize {
    let edge = edge_masks();
    let north = edge[North as usize].flood_fill(player_road_pieces);
    let south = edge[South as usize].flood_fill(player_road_pieces);
    let east = edge[East as usize].flood_fill(player_road_pieces);
    let west = edge[West as usize].flood_fill(player_road_pieces);

    if !(north & south).is_empty() || !(east & west).is_empty() {
        return 0;
    }

    // Take the first step
    let north = (north.dilate() | edge[North as usize]) & !blocking_pieces;
    let south = (south.dilate() | edge[South as usize]) & !blocking_pieces;
    let east = (east.dilate() | edge[East as usize]) & !blocking_pieces;
    let west = (west.dilate() | edge[West as usize]) & !blocking_pieces;

    fn get_path_steps<const M: usize>(
        start: Bitmap<M>,
        end: Bitmap<M>,
        mut roads: Bitmap<M>,
        blocks: Bitmap<M>,
    ) -> Option<usize> {
        roads &= !(start | end);

        let mut explored = Bitmap::empty();
        let mut next = start;
        let mut steps = 1;

        loop {
            if !(next & end).is_empty() {
                return Some(steps);
            }

            if next.is_empty() {
                return None;
            }

            explored |= next;
            next = next.dilate() & !(explored | blocks);
            steps += 1;

            if !(next & roads).is_empty() {
                let island = next.flood_fill(roads);
                next = next | (island.dilate() & !(explored | blocks));
                roads &= !next;
            }
        }
    }

    let vertical_steps =
        get_path_steps(north, south, player_road_pieces, blocking_pieces).unwrap_or(UNREACHABLE);
    let horizontal_steps =
        get_path_steps(east, west, player_road_pieces, blocking_pieces).unwrap_or(UNREACHABLE);

    vertical_steps.min(horizontal_steps)
}

/// Calculates a road-completion heuristic that is higher when a road is nearing completion, range [0.0, 1.0].
fn gather_road_steps<const N: usize>(
    player_road_pieces: Bitmap<N>,
    blocking_pieces: Bitmap<N>,
) -> f32 {
    let completion = N as f32 - calculate_road_steps(player_road_pieces, blocking_pieces) as f32;
    (completion / N as f32).max(0.0)
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
    fn road_steps() {
        let p: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        let b: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        assert_eq!(calculate_road_steps(p, b), 6);

        let p: Bitmap<6> = 0b100000_000000_000000_000000_000000_000000.into();
        let b: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        assert_eq!(calculate_road_steps(p, b), 5);

        let p: Bitmap<6> = 0b110000_000000_000000_000000_000000_000000.into();
        let b: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        assert_eq!(calculate_road_steps(p, b), 4);

        let p: Bitmap<6> = 0b000000_000000_001000_000000_000000_000000.into();
        let b: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        assert_eq!(calculate_road_steps(p, b), 5);

        let p: Bitmap<6> = 0b000000_000000_110101_000000_000000_000000.into();
        let b: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        assert_eq!(calculate_road_steps(p, b), 2);

        let p: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        let b: Bitmap<6> = 0b000001_000001_000001_000001_000001_111111.into();
        assert_eq!(calculate_road_steps(p, b), UNREACHABLE);

        let p: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        let b: Bitmap<6> = 0b000001_000001_000001_000001_000001_110111.into();
        assert_eq!(calculate_road_steps(p, b), 6);

        let p: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        let b: Bitmap<6> = 0b001001_000001_000001_000001_000001_110111.into();
        assert_eq!(calculate_road_steps(p, b), 7);

        let p: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        let b: Bitmap<6> = 0b001001_001001_000001_000001_000001_110111.into();
        assert_eq!(calculate_road_steps(p, b), 7);

        let p: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        let b: Bitmap<6> = 0b000001_000001_000001_000001_001001_110111.into();
        assert_eq!(calculate_road_steps(p, b), UNREACHABLE);

        let p: Bitmap<6> = 0b000000_000000_000000_000000_000000_000000.into();
        let b: Bitmap<6> = 0b101111_101000_101010_101010_100010_111110.into();
        assert_eq!(calculate_road_steps(p, b), 16);

        let p: Bitmap<6> = 0b001110_111010_000001_000011_000110_000100.into();
        let b: Bitmap<6> = 0b110001_000001_001110_110000_000000_001001.into();
        assert_eq!(calculate_road_steps(p, b), 3);
    }

    #[test]
    fn correct_features() {
        let state: State<6> = "2,1221122,1,1,1,2S/1,1,1,x,1C,1111212/x2,2,212,2C,11/2,2,x2,1,1/x3,1,1,x/x2,2,21,x,112S 1 32".parse().unwrap();
        let f = state.gather_features();
        let c = features_6s::Features {
            fcd: 4.0,
            player: features_6s::PlayerFeatures {
                reserve_flatstones: 6.0 / 30.0,
                stack_composition: StackComposition {
                    shallow_friendlies: [1.0, 0.0, 0.0],
                    shallow_captives: [1.0, 0.0, 0.0],
                    deep_friendlies: [0.0, 0.0, 0.0],
                    deep_captives: [0.0, 0.0, 0.0],
                },
                flatstone_positions: [0.0, 2.0, 5.0, 2.0, 3.0, 0.0],
                capstone_positions: [0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                road_groups: 2.0,
                lines_occupied: 12.0,
                standing_stone_surroundings: 0.0,
                capstone_surroundings: 1.0,
                unblocked_road_completion: 3.0 / 6.0,
                softblocked_road_completion: 5.0 / 6.0,
            },
            opponent: features_6s::PlayerFeatures {
                reserve_flatstones: 14.0 / 30.0,
                stack_composition: StackComposition {
                    shallow_friendlies: [5.0, 0.0, 0.0],
                    shallow_captives: [7.0, 2.0, 0.0],
                    deep_friendlies: [0.0, 0.0, 0.0],
                    deep_captives: [2.0, 0.0, 0.0],
                },
                flatstone_positions: [1.0, 2.0, 2.0, 0.0, 1.0, 2.0],
                capstone_positions: [0.0, 0.0, 0.0, 0.0, 1.0, 0.0],
                road_groups: 5.0,
                lines_occupied: 11.0,
                standing_stone_surroundings: 1.0,
                capstone_surroundings: 2.0,
                unblocked_road_completion: 0.0 / 6.0,
                softblocked_road_completion: 4.0 / 6.0,
            },
        };
        assert_eq!(f, c);

        let state: State<7> = "2,2,21S,2,1,1,1/2,1,x,2,1,x,1/2,2,2,2,21112C,121S,x/x2,1112C,2,1,1112S,x/121,22211C,1S,1,1,121,1221C/x,2,2,2,1,12,2/2,x3,1,122,x 2 50".parse().unwrap();
        let f = state.gather_features();
        let c = features_7s::Features {
            fcd: 4.0,
            player: features_7s::PlayerFeatures {
                reserve_flatstones: 11.0 / 40.0,
                stack_composition: StackComposition {
                    shallow_friendlies: [1.0, 0.0, 1.0],
                    shallow_captives: [2.0, 3.0, 6.0],
                    deep_friendlies: [0.0, 0.0, 0.0],
                    deep_captives: [0.0, 0.0, 0.0],
                },
                flatstone_positions: [2.0, 4.0, 1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0],
                capstone_positions: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0],
                road_groups: 4.0,
                lines_occupied: 13.0,
                standing_stone_surroundings: 2.0,
                capstone_surroundings: 2.0,
                unblocked_road_completion: 0.0 / 7.0,
                softblocked_road_completion: 5.0 / 7.0,
            },
            opponent: features_7s::PlayerFeatures {
                reserve_flatstones: 8.0 / 40.0,
                stack_composition: StackComposition {
                    shallow_friendlies: [2.0, 1.0, 2.0],
                    shallow_captives: [2.0, 2.0, 5.0],
                    deep_friendlies: [0.0, 0.0, 0.0],
                    deep_captives: [0.0, 0.0, 0.0],
                },
                flatstone_positions: [1.0, 2.0, 3.0, 0.0, 1.0, 3.0, 0.0, 1.0, 2.0, 0.0],
                capstone_positions: [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
                road_groups: 4.0,
                lines_occupied: 12.0,
                standing_stone_surroundings: 3.0,
                capstone_surroundings: 2.0,
                unblocked_road_completion: 5.0 / 7.0,
                softblocked_road_completion: 5.0 / 7.0,
            },
        };
        assert_eq!(f, c);
    }
}
