#![allow(clippy::comparison_chain)]

use tak::{Bitmap, Color, PieceType, Resolution, State};

pub use self::types::Evaluation;
pub(crate) use crate::util::placement_threat_map;

use self::types::EvalType;

mod types;

/// An evaluator that mimics Topaz's.
pub fn evaluate<const N: usize>(state: &State<N>, start_ply: u16) -> Evaluation {
    use Color::*;
    use PieceType::*;

    match state.resolution() {
        None => (),
        Some(Resolution::Road(color)) | Some(Resolution::Flats { color, .. }) => {
            if color == state.to_move() {
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

    for y in (0..N).rev() {
        for x in 0..N {
            let stack = &state.board[x][y];

            // Top piece bonus
            if let Some(piece) = stack.last() {
                let piece_type = match piece.piece_type() {
                    Flatstone => WEIGHT.pieces[0],
                    StandingStone => WEIGHT.pieces[1],
                    Capstone => WEIGHT.pieces[2],
                };

                let location = if piece.piece_type() == Capstone && stack.len() == 1 {
                    2 * WEIGHT.location[x][y]
                } else {
                    WEIGHT.location[x][y]
                };

                match piece.color() {
                    White => p1_eval += piece_type + location,
                    Black => p2_eval += piece_type + location,
                }
            }

            // Friendlies and Captives
            if stack.len() > 1 {
                let top_piece = stack.last().unwrap();
                let top_piece_color = top_piece.color();
                let top_piece_type = top_piece.piece_type();

                // Bonus for hard caps
                if top_piece_type == Capstone && stack.get(1).unwrap().color() == top_piece_color {
                    match top_piece_color {
                        White => p1_eval += 30,
                        Black => p2_eval += 30,
                    }
                }

                let mut safety = 0;
                let mut mobility = 0;

                match top_piece_type {
                    StandingStone => safety += 16,
                    Capstone => {
                        safety += 64;
                        mobility += 1;
                    }
                    _ => (),
                }

                let neighbors = {
                    let mut bit = Bitmap::<N>::default();
                    bit.set(x, y);
                    (bit.dilate() ^ bit)
                        .bits()
                        .map(|b| b.coordinates())
                        .map(|(x, y)| &state.board[x][y])
                };

                for neighbor in neighbors {
                    if let Some(neighbor_top) = neighbor.last() {
                        if neighbor_top.color() == top_piece_color {
                            match neighbor_top.piece_type() {
                                Flatstone => {
                                    safety += 1;
                                    mobility += 1;
                                }
                                StandingStone => {
                                    if neighbor.len() < N {
                                        safety += 4;
                                    }
                                }
                                Capstone => {
                                    if neighbor.len() < N {
                                        safety += 32;
                                    }
                                    mobility += 1;
                                }
                            }
                        } else {
                            match neighbor_top.piece_type() {
                                Flatstone => mobility += 2,
                                StandingStone => {
                                    if neighbor.len() < N {
                                        safety -= 4;
                                    }
                                }
                                Capstone => {
                                    if neighbor.len() < N {
                                        safety -= 32;
                                    }
                                }
                            }
                        }
                    } else {
                        mobility += 2;
                    }
                }

                let (player_stacks, opponent_stacks) = match top_piece_color {
                    White => (&m.p1_stacks, &m.p2_stacks),
                    Black => (&m.p2_stacks, &m.p1_stacks),
                };

                let (mut captives, mut friendlies) = (
                    opponent_stacks[x][y].count_ones() as EvalType,
                    player_stacks[x][y].count_ones() as EvalType - 1,
                );

                if mobility < 2 && top_piece_type == Flatstone {
                    friendlies /= 2;
                }

                if safety < 0 {
                    captives *= 2;
                }

                let (captive_mul, friendly_mul) = match top_piece_type {
                    Flatstone => WEIGHT.piece_mul[0],
                    StandingStone => WEIGHT.piece_mul[1],
                    Capstone => WEIGHT.piece_mul[2],
                };

                let stack_score = captives * captive_mul + friendlies * friendly_mul;
                match top_piece_color {
                    White => p1_eval += stack_score,
                    Black => p2_eval += stack_score,
                }
            }
        }
    }

    let p1_caps = m.capstones & m.p1_pieces;
    if p1_caps != 0.into() {
        let neighbors = p1_caps.dilate() & (m.flatstones | m.standing_stones);
        if neighbors == 0.into() {
            p2_eval += 30;
        }
    }

    let p2_caps = m.capstones & m.p1_pieces;
    if p2_caps != 0.into() {
        let neighbors = p2_caps.dilate() & (m.flatstones | m.standing_stones);
        if neighbors == 0.into() {
            p1_eval += 30;
        }
    }

    let critical_spaces = placement_threat_map(p1_road_pieces, m.p2_pieces & !m.flatstones);
    for space in critical_spaces.bits() {
        if ((space.dilate() ^ space) & p1_road_pieces).count_ones() >= 3 {
            p1_eval += 50;
        }
    }

    let critical_spaces = placement_threat_map(p2_road_pieces, m.p1_pieces & !m.flatstones);
    for space in critical_spaces.bits() {
        if ((space.dilate() ^ space) & p2_road_pieces).count_ones() >= 3 {
            p2_eval += 50;
        }
    }

    p1_eval -= p1_road_pieces.groups().count() as EvalType * WEIGHT.connectivity;
    p2_eval -= p2_road_pieces.groups().count() as EvalType * WEIGHT.connectivity;

    // XXX Road heuristic would go here.

    let p1_res = state.p1_flatstones + state.p1_capstones;
    let p2_res = state.p2_flatstones + state.p2_capstones;

    if p1_res < 10 || p2_res < 10 {
        let mut p1_flats = 2 * m.p1_flat_count as EvalType;
        let mut p2_flats = 2 * m.p2_flat_count as EvalType + state.komi.as_half_komi() as EvalType;

        if p1_res < p2_res {
            if state.to_move() == White {
                p1_flats += 2;
            }
        } else if p2_res < p1_res && state.to_move() == Black {
            p2_flats += 2;
        }

        if p1_flats > p2_flats {
            p1_eval += (p1_flats - p2_flats) * 100 / p1_res as EvalType;
        } else if p2_flats > p1_flats {
            p2_eval += (p2_flats - p1_flats) * 100 / p2_res as EvalType;
        }
    }

    let depth = state.ply_count - start_ply;
    if depth % 2 == 1 {
        match state.to_move() {
            White => p1_eval += WEIGHT.tempo_offset,
            Black => p2_eval += WEIGHT.tempo_offset,
        }
    }

    match state.to_move() {
        White => p1_eval - p2_eval,
        Black => p2_eval - p1_eval,
    }
}

struct Weights {
    pieces: [EvalType; 3],
    location: [[EvalType; 6]; 6],
    piece_mul: [(EvalType, EvalType); 3],
    connectivity: EvalType,
    tempo_offset: EvalType,
}

const WEIGHT: Weights = Weights {
    pieces: [100, 40, 80],
    location: [
        [0, 5, 5, 5, 5, 0],
        [5, 10, 15, 15, 10, 5],
        [5, 15, 20, 20, 15, 5],
        [5, 15, 20, 20, 15, 5],
        [5, 10, 15, 15, 10, 5],
        [0, 5, 5, 5, 5, 0],
    ],
    piece_mul: [(-50, 60), (-30, 70), (-20, 90)],
    connectivity: 20,
    tempo_offset: 150,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_eval() {
        let state: State<6> = "2,212221C,2,2,2C,1/1,2,1,1,2,1/12,x,1S,2S,2,1/2,2,2,x2,1/1,2212121S,2,12,1,1S/x,2,2,2,x,1 1 30".parse().unwrap();

        println!("{:?}", evaluate(&state, state.ply_count - 5));
    }
}
