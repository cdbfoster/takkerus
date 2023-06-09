//! Features for models with capstones (5s - 8s).

macro_rules! features_large_impl {
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
                + 2                     // Unblocked road completion
                + 2                     // Soft-blocked road completion
                + 2 * 2                 // Standing stone blockage of standing stone and flatstone stacks
                + 2 * 3                 // Capstone blockage of stacks for each piece type
                ;

            #[repr(C)]
            #[derive(Debug, Default, PartialEq)]
            pub struct PlayerFeatures {
                pub reserve_flatstones: f32,
                pub stack_composition: StackComposition<3>,
                pub flatstone_positions: [f32; POSITIONS],
                pub capstone_positions: [f32; POSITIONS],
                pub road_groups: f32,
                pub lines_occupied: f32,
                pub unblocked_road_completion: f32,
                pub softblocked_road_completion: f32,
                pub standing_stone_blockage: [f32; 2],
                pub capstone_blockage: [f32; 3],
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

                    p1.unblocked_road_completion = gather_road_steps(p1_road_pieces, all_pieces & !p1_road_pieces);
                    p2.unblocked_road_completion = gather_road_steps(p2_road_pieces, all_pieces & !p2_road_pieces);

                    p1.softblocked_road_completion = gather_road_steps(p1_road_pieces, p1_standing_stones | p2_standing_stones | p2_capstones);
                    p2.softblocked_road_completion = gather_road_steps(p2_road_pieces, p2_standing_stones | p1_standing_stones | p1_capstones);

                    p1.standing_stone_blockage = gather_stack_blockage(p1_standing_stones, [p2_flatstones, p2_standing_stones], self);
                    p2.standing_stone_blockage = gather_stack_blockage(p2_standing_stones, [p1_flatstones, p1_standing_stones], self);

                    p1.capstone_blockage = gather_stack_blockage(p1_capstones, [p2_flatstones, p2_standing_stones, p2_capstones], self);
                    p2.capstone_blockage = gather_stack_blockage(p2_capstones, [p1_flatstones, p1_standing_stones, p1_capstones], self);

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

pub(super) use features_large_impl;
