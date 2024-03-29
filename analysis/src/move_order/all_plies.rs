use tak::{board_mask, edge_masks, generation, Bitmap, Color, Direction, PieceType, Ply, State};

use crate::ply_generator::Continuation::*;
use crate::ply_generator::Fallibility::*;
use crate::ply_generator::GeneratedPly;

use Color::*;
use Direction::*;
use PieceType::*;

pub(crate) struct AllPlies<'a, const N: usize> {
    state: &'a State<N>,
    plies: Option<Vec<ScoredPly<N>>>,
}

impl<'a, const N: usize> AllPlies<'a, N> {
    pub fn new(state: &'a State<N>) -> Self {
        Self { state, plies: None }
    }
}

impl<'a, const N: usize> Iterator for AllPlies<'a, N> {
    type Item = GeneratedPly<N>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(plies) = &mut self.plies {
            plies.pop().map(|scored_ply| GeneratedPly {
                ply: scored_ply.ply,
                fallibility: Infallible,
                continuation: Continue,
            })
        } else {
            let mut plies = generate_all_plies(self.state);

            // Don't really need a detailed ordering so early in the game.
            if self.state.ply_count >= 6 {
                score_plies(self.state, &mut plies);
                plies.sort_unstable_by_key(|scored_ply| scored_ply.score);
            }

            self.plies = Some(plies);
            self.next()
        }
    }
}

struct ScoredPly<const N: usize> {
    score: u32,
    ply: Ply<N>,
}

// Generate all available plies in a simple, but probably beneficial order.
fn generate_all_plies<const N: usize>(state: &State<N>) -> Vec<ScoredPly<N>> {
    let mut plies = Vec::new();

    let empty = board_mask() ^ state.metadata.p1_pieces ^ state.metadata.p2_pieces;

    let reserve_flatstones = match state.to_move() {
        White => state.p1_flatstones,
        Black => state.p2_flatstones,
    };

    if state.ply_count >= 2 {
        let player_stacks = match state.to_move() {
            White => state.metadata.p1_pieces,
            Black => state.metadata.p2_pieces,
        };

        // Standing stones.
        if reserve_flatstones > 0 {
            plies.extend(
                generation::placements(empty, StandingStone).map(|ply| ScoredPly { score: 0, ply }),
            );
        }

        // Spreads.
        plies.extend(
            generation::spreads(state, player_stacks).map(|ply| ScoredPly { score: 0, ply }),
        );

        let reserve_capstones = match state.to_move() {
            White => state.p1_capstones,
            Black => state.p2_capstones,
        };

        // Capstones.
        if reserve_capstones > 0 {
            plies.extend(
                generation::placements(empty, Capstone).map(|ply| ScoredPly { score: 0, ply }),
            );
        }
    }

    // Flatstones.
    if reserve_flatstones > 0 {
        plies.extend(
            generation::placements(empty, Flatstone).map(|ply| ScoredPly { score: 0, ply }),
        );
    }

    plies
}

/// Scores plies to achieve the following order (greatest to least):
/// - Capstone placements that create road threats
/// - Flatstone placements that create road threats
/// - Flatstone placements
/// - Capstone placements next to opponent roads
/// - Standing stone placements next to opponent roads
/// - Capstone placements
/// - Spreads with capstones that don't reveal opponent pieces and increase fcd
/// - Spreads with standing stones that don't reveal opponent pieces and increase fcd
/// - Spreads with flatstones that don't reveal opponent pieces and increase fcd
/// - Spreads with capstones that don't reveal opponent pieces
/// - Spreads with standing stones that don't reveal opponent pieces
/// - Spreads with flatstones that don't reveal opponent pieces
/// - Spreads
/// - Standing stone placements
fn score_plies<const N: usize>(state: &State<N>, plies: &mut [ScoredPly<N>]) {
    const ROAD_THREAT: u32 = 1 << 11;
    const ROAD_THREAT_CAPSTONE: u32 = 1 << 10;
    const FLATSTONE: u32 = 1 << 9;
    const BLOCKER_NEAR_OPPONENT_ROAD: u32 = 1 << 8;
    const BLOCKER_CAPSTONE_NEAR_OPPONENT_ROAD: u32 = 1 << 7;
    const CAPSTONE: u32 = 1 << 6;
    const SPREAD: u32 = 1 << 5;
    const SPREAD_INCREASES_FCD: u32 = 1 << 4;
    const SPREAD_DOESNT_REVEAL_OPPONENT: u32 = 1 << 3;
    const SPREAD_CAPSTONE: u32 = 1 << 2;
    const SPREAD_STANDING_STONE: u32 = 1 << 1;
    const STANDING_STONE: u32 = 1 << 0;

    let m = &state.metadata;

    let all_pieces = m.p1_pieces | m.p2_pieces;

    let player_pieces = match state.to_move() {
        White => m.p1_pieces,
        Black => m.p2_pieces,
    };
    let opponent_pieces = all_pieces ^ player_pieces;
    let opponent_flatstones = opponent_pieces & m.flatstones;

    let road_pieces = (m.flatstones | m.capstones) & player_pieces;
    let blocking_pieces = all_pieces ^ road_pieces;

    // Basically an inlined placement_threat_maps to avoid all the flood fills per ply.
    let e = edge_masks();
    let left = e[West as usize].flood_fill(road_pieces).dilate() | e[West as usize];
    let right = e[East as usize].flood_fill(road_pieces).dilate() | e[East as usize];
    let top = e[North as usize].flood_fill(road_pieces).dilate() | e[North as usize];
    let bottom = e[South as usize].flood_fill(road_pieces).dilate() | e[South as usize];

    let placement_threat = |bit: Bitmap<N>| {
        let dilated = bit.dilate();

        let next_left = if !(bit & left).is_empty() {
            left | dilated
        } else {
            left
        };
        let next_right = if !(bit & right).is_empty() {
            right | dilated
        } else {
            right
        };
        let next_top = if !(bit & top).is_empty() {
            top | dilated
        } else {
            top
        };
        let next_bottom = if !(bit & bottom).is_empty() {
            bottom | dilated
        } else {
            bottom
        };

        let horizontal = next_left & next_right;
        let vertical = next_top & next_bottom;

        let threats = (horizontal | vertical) & !blocking_pieces;

        !threats.is_empty()
    };

    for ScoredPly { score, ply } in plies {
        match *ply {
            Ply::Place { x, y, piece_type } => {
                *score |= match piece_type {
                    Flatstone => FLATSTONE,
                    StandingStone => STANDING_STONE,
                    Capstone => CAPSTONE,
                };

                // Road threats
                if piece_type == Capstone || piece_type == Flatstone {
                    let bit = Bitmap::from_coordinates(x as usize, y as usize);

                    if placement_threat(bit) {
                        *score |= ROAD_THREAT;

                        if piece_type == Capstone {
                            *score |= ROAD_THREAT_CAPSTONE;
                        }
                    }
                }

                // Blockers near opponent roads
                if piece_type == Capstone || piece_type == StandingStone {
                    let bit = Bitmap::from_coordinates(x as usize, y as usize);
                    let neighbors = bit.dilate() ^ bit;

                    if !(neighbors & opponent_flatstones).is_empty() {
                        *score |= BLOCKER_NEAR_OPPONENT_ROAD;

                        if piece_type == Capstone {
                            *score |= BLOCKER_CAPSTONE_NEAR_OPPONENT_ROAD;
                        }
                    }
                }
            }
            Ply::Spread {
                x,
                y,
                direction,
                drops,
                ..
            } => {
                *score |= SPREAD;

                let mut carry = drops.iter().sum::<u8>() as usize;
                let stack = &state.board[x as usize][y as usize];

                match stack.top_piece_type() {
                    Some(Capstone) => *score |= SPREAD_CAPSTONE,
                    Some(StandingStone) => *score |= SPREAD_STANDING_STONE,
                    _ => (),
                }

                let mut delta_fcd = 0;
                let mut reveals_opponent = false;

                // Check the color of the stone that's left behind.
                if stack.len() > carry {
                    let revealed_color = stack.get(carry).unwrap().color();
                    if revealed_color != state.to_move() {
                        delta_fcd -= 1;
                        reveals_opponent = true;
                    } else {
                        delta_fcd += 1;
                    }
                }

                let player_pieces = match state.to_move() {
                    White => stack.get_player_bitmaps().0,
                    Black => stack.get_player_bitmaps().1,
                };

                let mut mask = 0x80 >> (8 - carry);

                let (mut tx, mut ty) = (x as i8, y as i8);
                let (dx, dy) = direction.to_offset();

                for drop in drops.iter() {
                    // Check the color of the stone we're covering.
                    tx += dx;
                    ty += dy;
                    let covered_piece = state.board[tx as usize][ty as usize].top();
                    if let Some(piece) = covered_piece {
                        if piece.piece_type() == Flatstone {
                            if piece.color() == state.to_move() {
                                delta_fcd -= 1;
                            } else {
                                delta_fcd += 1;
                            }
                        }
                    }

                    carry -= drop as usize;

                    if carry == 0 {
                        break;
                    }

                    // Check the color of the top dropped stone.
                    //player_pieces >>= drop - 1;
                    mask >>= drop - 1;
                    if player_pieces & mask == 1 {
                        delta_fcd += 1;
                    } else {
                        delta_fcd -= 1;
                        reveals_opponent = true;
                    }
                    mask >>= 1;
                }

                if delta_fcd > 0 {
                    *score |= SPREAD_INCREASES_FCD;
                }

                if !reveals_opponent {
                    *score |= SPREAD_DOESNT_REVEAL_OPPONENT;
                }
            }
        }
    }
}
