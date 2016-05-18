//
// This file is part of Takkerus.
//
// Takkerus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Takkerus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Takkerus. If not, see <http://www.gnu.org/licenses/>.
//
// Copyright 2016 Chris Foster
//

lazy_static! {
    pub static ref SLIDE_TABLE: Vec<Vec<Vec<u8>>> = generate_slide_table(8);
}

use tak::{Color, GameError, Piece, Ply, Seat, StateAnalysis};
use tak::state_analysis::BitmapInterface;

#[derive(Clone, Debug)]
pub struct State {
    pub p1: Seat,
    pub p2: Seat,

    pub board: Vec<Vec<Vec<Piece>>>,

    pub ply_count: u16,
    pub analysis: StateAnalysis,
}

impl State {
    pub fn new(board_size: usize) -> State {
        let (flatstone_count, capstone_count) = match board_size {
            3 => (10, 0),
            4 => (15, 0),
            5 => (21, 1),
            6 => (30, 1),
            7 => (40, 1),
            8 => (50, 2),
            _ => panic!("Illegal board size!"),
        };

        State {
            p1: Seat::new(Color::White, flatstone_count, capstone_count),
            p2: Seat::new(Color::Black, flatstone_count, capstone_count),
            board: vec![vec![Vec::new(); board_size]; board_size],
            ply_count: 0,
            analysis: StateAnalysis::new(),
        }
    }

    pub fn execute_ply(&self, ply: &Ply) -> Result<State, GameError> {
        let mut next = self.clone();
        next.ply_count += 1;

        let board_size = next.board.len();

        match ply {
            &Ply::Place { x, y, ref piece } => {
                if !next.board[x][y].is_empty() {
                    return Err(GameError::IllegalPlacement);
                }

                let count = match piece {
                    &Piece::Flatstone(color) |
                    &Piece::StandingStone(color) => if color == Color::White {
                        &mut next.p1.flatstone_count
                    } else {
                        &mut next.p2.flatstone_count
                    },
                    &Piece::Capstone(color) => if color == Color::White {
                        &mut next.p1.capstone_count
                    } else {
                        &mut next.p2.capstone_count
                    },
                };

                if *count > 0 {
                    *count -= 1;
                } else {
                    return Err(GameError::InsufficientPieces);
                }

                next.board[x][y].push(piece.clone());

                let a = &mut next.analysis;
                match piece {
                    &Piece::Flatstone(color) => if color == Color::White {
                        a.p1_flatstone_count += 1;
                        a.p1_total_road.set(x, y, board_size);
                        a.p1_road_groups = a.p1_total_road.get_groups(board_size);
                        a.p1_pieces.set(x, y, board_size);
                    } else {
                        a.p2_flatstone_count += 1;
                        a.p2_total_road.set(x, y, board_size);
                        a.p2_road_groups = a.p2_total_road.get_groups(board_size);
                        a.p2_pieces.set(x, y, board_size);
                    },
                    &Piece::Capstone(color) => if color == Color::White {
                        a.p1_total_road.set(x, y, board_size);
                        a.p1_road_groups = a.p1_total_road.get_groups(board_size);
                        a.p1_pieces.set(x, y, board_size);
                    } else {
                        a.p2_total_road.set(x, y, board_size);
                        a.p2_road_groups = a.p2_total_road.get_groups(board_size);
                        a.p2_pieces.set(x, y, board_size);
                    },
                    &Piece::StandingStone(color) => if color == Color::White {
                        a.p1_pieces.set(x, y, board_size);
                    } else {
                        a.p2_pieces.set(x, y, board_size);
                    },
                }
            },
            &Ply::Slide { x, y, direction, ref drops } => {
                let grab = drops.iter().fold(0, |acc, x| acc + x);

                if grab > board_size || next.board[x][y].is_empty() {
                    return Err(GameError::IllegalSlide);
                }

                let mut stack = Vec::new();
	            for _ in 0..grab {
	                stack.push(next.board[x][y].pop().unwrap());
                }

                let a = &mut next.analysis;
                match stack[0] {
                    Piece::Flatstone(color) => if color == Color::White {
                        a.p1_flatstone_count -= 1;
                        a.p1_total_road.clear(x, y, board_size);
                        a.p1_pieces.clear(x, y, board_size);
                    } else {
                        a.p2_flatstone_count -= 1;
                        a.p2_total_road.clear(x, y, board_size);
                        a.p2_pieces.clear(x, y, board_size);
                    },
                    Piece::Capstone(color) => if color == Color::White {
                        a.p1_total_road.clear(x, y, board_size);
                        a.p1_pieces.clear(x, y, board_size);
                    } else {
                        a.p2_total_road.clear(x, y, board_size);
                        a.p2_pieces.clear(x, y, board_size);
                    },
                    Piece::StandingStone(color) => if color == Color::White {
                        a.p1_pieces.clear(x, y, board_size);
                    } else {
                        a.p2_pieces.clear(x, y, board_size);
                    },
                }

                let (dx, dy) = direction.to_offset();

                let mut nx = x as i8;
                let mut ny = y as i8;
                for drop in drops {
                    nx += dx;
                    ny += dy;

                    if nx < 0 || nx >= board_size as i8 ||
                       ny < 0 || ny >= board_size as i8 {
                        return Err(GameError::OutOfBounds);
                    }

                    if !next.board[nx as usize][ny as usize].is_empty() {
                        let target_top = next.board[nx as usize][ny as usize].last().unwrap().clone();
                        match  target_top {
                            Piece::Flatstone(color) => if color == Color::White {
                                a.p1_flatstone_count -= 1;
                                a.p1_total_road.clear(nx as usize, ny as usize, board_size);
                                a.p1_pieces.clear(nx as usize, ny as usize, board_size);
                            } else {
                                a.p2_flatstone_count -= 1;
                                a.p2_total_road.clear(nx as usize, ny as usize, board_size);
                                a.p2_pieces.clear(nx as usize, ny as usize, board_size);
                            },
                            Piece::Capstone(_) => return Err(GameError::IllegalSlide),
                            Piece::StandingStone(color) => if stack.len() == 1 {
                                match stack[0] {
                                    Piece::Capstone(_) => {
                                        *next.board[nx as usize][ny as usize].last_mut().unwrap() = Piece::Flatstone(color);
                                        if color == Color::White {
                                            a.p1_pieces.clear(nx as usize, ny as usize, board_size);
                                        } else {
                                            a.p2_pieces.clear(nx as usize, ny as usize, board_size);
                                        }
                                    },
                                    _ => return Err(GameError::IllegalSlide),
                                }
                            } else {
                                return Err(GameError::IllegalSlide);
                            },
                        }
                    }

                    for _ in 0..*drop {
                        next.board[nx as usize][ny as usize].push(stack.pop().unwrap());
                    }

                    match next.board[nx as usize][ny as usize].last().unwrap() {
                        &Piece::Flatstone(color) => if color == Color::White {
                            a.p1_flatstone_count += 1;
                            a.p1_total_road.set(nx as usize, ny as usize, board_size);
                            a.p1_pieces.set(nx as usize, ny as usize, board_size);
                        } else {
                            a.p2_flatstone_count += 1;
                            a.p2_total_road.set(nx as usize, ny as usize, board_size);
                            a.p2_pieces.set(nx as usize, ny as usize, board_size);
                        },
                        &Piece::Capstone(color) => if color == Color::White {
                            a.p1_total_road.set(nx as usize, ny as usize, board_size);
                            a.p1_pieces.set(nx as usize, ny as usize, board_size);
                        } else {
                            a.p2_total_road.set(nx as usize, ny as usize, board_size);
                            a.p2_pieces.set(nx as usize, ny as usize, board_size);
                        },
                        &Piece::StandingStone(color) => if color == Color::White {
                            a.p1_pieces.set(nx as usize, ny as usize, board_size);
                        } else {
                            a.p2_pieces.set(nx as usize, ny as usize, board_size);
                        },
                    }
                }

                a.p1_road_groups = a.p1_total_road.get_groups(board_size);
                a.p2_road_groups = a.p2_total_road.get_groups(board_size);
            },
        }

        Ok(next)
    }
}

fn generate_slide_table(size: u8) -> Vec<Vec<Vec<u8>>> {
    let mut result: Vec<Vec<Vec<u8>>> = Vec::with_capacity(size as usize);
    result.push(Vec::new());

    for stack in 1..(size + 1) {
        let mut out = Vec::with_capacity((2 as usize).pow(stack as u32) - 1);

        for i in 1..(stack + 1) {
            out.push(vec![i]);

            for sub in &result[(stack - i) as usize] {
                let mut t = vec![0; sub.len() + 1];
                t[0] = i;
                t[1..].clone_from_slice(sub);

                out.push(t);
            }
        }

        result.push(out);
    }

    result
}
