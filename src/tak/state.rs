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

use std::fmt::{self, Write};
use std::str::FromStr;

use tak::{Color, GameError, Piece, Ply, Seat, StateAnalysis, Win};
use tak::state_analysis::{BOARD, EDGE, Bitmap};

#[derive(Debug)]
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
            s => panic!("Illegal board size: {}", s),
        };

        State {
            p1: Seat::new(Color::White, flatstone_count, capstone_count),
            p2: Seat::new(Color::Black, flatstone_count, capstone_count),
            board: vec![vec![Vec::new(); board_size]; board_size],
            ply_count: 0,
            analysis: StateAnalysis::new(board_size),
        }
    }

    pub fn from_plies(size: usize, plies: &[Ply]) -> Option<State> {
        let mut state = State::new(size);

        for ply in plies {
            match state.execute_ply(ply) {
                Ok(next) => state = next,
                Err(_) => return None,
            }
        }

        Some(state)
    }

    pub fn from_tps(tps: &str) -> Option<State> {
        if &tps[0..6] != "[TPS \"" || &tps[(tps.len() - 2)..] != "\"]" {
            return None;
        }

        let mut chars = tps[6..(tps.len() - 2)].chars();

        let mut x = 0;
        let mut y = 0;
        let mut board: Vec<Vec<Vec<Piece>>> = Vec::new();
        let mut piece_color = None;

        let mut p1_used_flatstones = 0;
        let mut p1_used_capstones = 0;

        let mut p2_used_flatstones = 0;
        let mut p2_used_capstones = 0;

        fn ensure_dimensions(board: &mut Vec<Vec<Vec<Piece>>>, x: usize, y: usize) {
            if x >= board.len() {
                for _ in board.len()..(x + 1) {
                    board.push(Vec::new());
                }
            }

            for column in board.iter_mut() {
                if y >= column.len() {
                    for _ in column.len()..(y + 1) {
                        column.push(Vec::new());
                    }
                }
            }
        }

        let mut next = chars.next();
        while next.is_some() {
            ensure_dimensions(&mut board, x, y);

            match piece_color {
                Some(color) => {
                    let piece = match next {
                        Some('S') => Piece::StandingStone(color),
                        Some('C') => Piece::Capstone(color),
                        _ => Piece::Flatstone(color),
                    };

                    let (used_flatstones, used_capstones) = match color {
                        Color::White => (&mut p1_used_flatstones, &mut p1_used_capstones),
                        Color::Black => (&mut p2_used_flatstones, &mut p2_used_capstones),
                    };

                    match piece {
                        Piece::Capstone(_) => *used_capstones += 1,
                        _ => *used_flatstones += 1,
                    }

                    board[x][y].push(piece);

                    piece_color = None;
                    match next {
                        Some('S') |
                        Some('C') => next = chars.next(),
                        _ => (),
                    }
                },
                None => (),
            }

            match next {
                Some('x') => match chars.next() {
                    Some(c) => if c.is_digit(10) {
                        x += (c as u8 - 49) as usize;
                    } else if c == ',' {
                        x += 1;
                    } else if c == '/' {
                        x = 0;
                        y += 1;
                    } else if c == ' ' {
                        break;
                    } else {
                        return None;
                    },
                    _ => return None,
                },
                Some(',') => {
                    x += 1;
                },
                Some('/') => {
                    x = 0;
                    y += 1;
                },
                Some(' ') => break,
                Some('1') => piece_color = Some(Color::White),
                Some('2') => piece_color = Some(Color::Black),
                _ => return None,
            }

            next = chars.next();
        }

        let ply_count = {
            let player = match chars.next() {
                Some('1') => 0,
                Some('2') => 1,
                _ => return None,
            };

            chars.next();

            let turn_count = match u16::from_str(chars.as_str()) {
                Ok(c) => if c > 0 {
                    c - 1
                } else {
                    return None
                },
                _ => return None,
            };

            turn_count * 2 + player
        };

        for column in board.iter_mut() {
            column.reverse();
        }

        let mut state = State::new(board.len());
        state.p1.flatstone_count -= p1_used_flatstones;
        state.p1.capstone_count -= p1_used_capstones;
        state.p2.flatstone_count -= p2_used_flatstones;
        state.p2.capstone_count -= p2_used_capstones;
        state.board = board;
        state.ply_count = ply_count;
        state.update_analysis();

        Some(state)
    }

    pub fn execute_ply(&self, ply: &Ply) -> Result<State, GameError> {
        let mut next = self.clone();
        match self.execute_ply_preallocated(ply, &mut next) {
            Ok(_) => Ok(next),
            Err(error) => Err(error),
        }
    }

    pub fn execute_ply_preallocated(&self, ply: &Ply, next: &mut State) -> Result<(), GameError> {
        next.clone_from(self);
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

                match piece {
                    &Piece::Flatstone(color) => next.analysis.add_flatstone(
                        color, x, y, next.board[x][y].len() - 1,
                    ),
                    block => next.analysis.add_blocking_stone(block, x, y),
                }

                match piece {
                    &Piece::Flatstone(_) |
                    &Piece::Capstone(_) => next.analysis.calculate_road_groups(),
                    _ => (),
                }
            },
            &Ply::Slide { x, y, direction, ref drops } => {
                let next_color = if self.ply_count % 2 == 0 {
                    Color::White
                } else {
                    Color::Black
                };

                match next.board[x][y].last() {
                    Some(&Piece::Flatstone(color)) |
                    Some(&Piece::StandingStone(color)) |
                    Some(&Piece::Capstone(color)) => if color != next_color {
                        return Err(GameError::IllegalMove);
                    },
                    _ => (),
                }

                let grab = drops.iter().fold(0, |acc, x| acc + x) as usize;

                if grab > board_size || next.board[x][y].len() < grab {
                    return Err(GameError::IllegalSlide);
                }

                let mut stack = Vec::new();
	            for _ in 0..grab {
	                let piece = next.board[x][y].pop().unwrap();

	                match piece {
	                    Piece::Flatstone(color) => next.analysis.remove_flatstone(
	                        color, x, y, next.board[x][y].len(),
                        ),
                        ref block => next.analysis.remove_blocking_stone(block, x, y),
                    }

                    match next.board[x][y].last() {
                        Some(revealed) => next.analysis.reveal_flatstone(
                            revealed.get_color(), x, y,
                        ),
                        None => (),
                    }

	                stack.push(piece);
                }

                let (dx, dy) = direction.to_offset();

                let mut nx = x as i8;
                let mut ny = y as i8;

                {
                    let (tx, ty) = (
                        nx + dx * drops.len() as i8,
                        ny + dy * drops.len() as i8,
                    );

                    if tx < 0 || tx >= board_size as i8 ||
                       ty < 0 || ty >= board_size as i8 {
                        return Err(GameError::OutOfBounds);
                    }
                }

                for drop in drops {
                    nx += dx;
                    ny += dy;

                    if !next.board[nx as usize][ny as usize].is_empty() {
                        let target_top = next.board[nx as usize][ny as usize].last().unwrap().clone();
                        match target_top {
                            Piece::Capstone(_) => return Err(GameError::IllegalSlide),
                            Piece::StandingStone(color) => if stack.len() == 1 {
                                match stack[0] {
                                    Piece::Capstone(_) => {
                                        *next.board[nx as usize][ny as usize].last_mut().unwrap() = Piece::Flatstone(color);
                                        next.analysis.remove_blocking_stone(&Piece::StandingStone(color), nx as usize, ny as usize);
                                        next.analysis.add_flatstone(
                                            color, nx as usize, ny as usize,
                                            next.board[nx as usize][ny as usize].len() - 1,
                                        )
                                    },
                                    _ => return Err(GameError::IllegalSlide),
                                }
                            } else {
                                return Err(GameError::IllegalSlide);
                            },
                            _ => (),
                        }
                    }

                    for _ in 0..*drop {
                        match next.board[nx as usize][ny as usize].last() {
                            Some(covered) => next.analysis.cover_flatstone(
                                covered.get_color(), nx as usize, ny as usize,
                            ),
                            None => (),
                        }

                        let piece = stack.pop().unwrap();

                        match piece {
                            Piece::Flatstone(color) => next.analysis.add_flatstone(
                                color, nx as usize, ny as usize,
                                next.board[nx as usize][ny as usize].len(),
                            ),
                            ref block => next.analysis.add_blocking_stone(
                                block, nx as usize, ny as usize,
                            ),
                        }

                        next.board[nx as usize][ny as usize].push(piece);
                    }
                }

                next.analysis.calculate_road_groups();
            },
        }

        Ok(())
    }

    pub fn check_win(&self) -> Win {
        let board_size = self.board.len();
        let a = &self.analysis;

        let has_road = |groups: &Vec<Bitmap>| {
            use tak::Direction::*;

            for group in groups.iter() {
                if (group & EDGE[board_size][North as usize] != 0 &&
                    group & EDGE[board_size][South as usize] != 0) ||
                   (group & EDGE[board_size][West as usize] != 0 &&
                    group & EDGE[board_size][East as usize] != 0) {
                    return true;
                }
            }

            false
        };

        let p1_has_road = has_road(&a.p1_road_groups);
        let p2_has_road = has_road(&a.p2_road_groups);

        if p1_has_road && p2_has_road {
            if self.ply_count % 2 == 1 {
                Win::Road(Color::White)
            } else {
                Win::Road(Color::Black)
            }
        } else if p1_has_road {
            Win::Road(Color::White)
        } else if p2_has_road {
            Win::Road(Color::Black)
        } else if (self.p1.flatstone_count + self.p1.capstone_count) == 0 ||
                  (self.p2.flatstone_count + self.p2.capstone_count) == 0 ||
                  (a.p1_pieces | a.p2_pieces) == BOARD[board_size] {
            if a.p1_flatstone_count > a.p2_flatstone_count {
                Win::Flat(Color::White)
            } else if a.p2_flatstone_count > a.p1_flatstone_count {
                Win::Flat(Color::Black)
            } else {
                Win::Draw
            }
        } else {
            Win::None
        }
    }

    pub fn update_analysis(&mut self) {
        let board_size = self.board.len();

        self.analysis = StateAnalysis::new(board_size);

        for x in 0..board_size {
            for y in 0..board_size {
                for z in 0..self.board[x][y].len() {
                    if z > 0 {
                        match self.board[x][y][z - 1] {
                            Piece::Flatstone(color) => self.analysis.cover_flatstone(
                                color, x, y,
                            ),
                            ref block => self.analysis.remove_blocking_stone(block, x, y),
                        }
                    }

                    match self.board[x][y][z] {
                        Piece::Flatstone(color) => self.analysis.add_flatstone(
                            color, x, y, z,
                        ),
                        ref block => self.analysis.add_blocking_stone(block, x, y),
                    }
                }
            }
        }

        self.analysis.calculate_road_groups();
    }

    pub fn get_signature(&self) -> StateSignature {
        StateSignature {
            next_color: if self.ply_count % 2 == 0 {
                Color::White
            } else {
                Color::Black
            },
            p1_flatstones: self.analysis.p1_flatstones.clone(),
            p2_flatstones: self.analysis.p2_flatstones.clone(),
            standing_stones: self.analysis.standing_stones,
            capstones: self.analysis.capstones,
            p1_pieces: self.analysis.p1_pieces,
            p2_pieces: self.analysis.p2_pieces,
        }
    }
}

impl Clone for State {
    fn clone(&self) -> State {
        State {
            p1: self.p1,
            p2: self.p2,
            board: self.board.clone(),
            ply_count: self.ply_count,
            analysis: self.analysis.clone(),
        }
    }

    fn clone_from(&mut self, source: &State) {
        self.p1 = source.p1;
        self.p2 = source.p2;
        self.board.clone_from(&source.board);
        self.ply_count = source.ply_count;
        self.analysis.clone_from(&source.analysis);
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let board_size = self.board.len();

        let column_widths = self.board.iter().map(|column| {
            column.iter().fold(6, |max, stack| {
                let stack_width = stack.iter().fold(0, |acc, piece| {
                    match piece {
                        &Piece::Flatstone(_) => acc + 1,
                        _ => acc + 2,
                    }
                }) + 3 + if !stack.is_empty() {
                    stack.len() - 1
                } else {
                    0
                };

                if max > stack_width { max } else { stack_width }
            })
        }).collect::<Vec<_>>();

        write!(f, "\n Player 1: {:>2} flatstone{}", self.p1.flatstone_count,
            if self.p1.flatstone_count != 1 { "s" } else { "" }
        ).ok();

        if self.p1.capstone_count > 0 {
            write!(f, ", {} capstone{}", self.p1.capstone_count,
                if self.p1.capstone_count != 1 { "s" } else { "" }
            ).ok();
        }

        write!(f, "\n Player 2: {:>2} flatstone{}", self.p2.flatstone_count,
            if self.p2.flatstone_count != 1 { "s" } else { "" }
        ).ok();

        if self.p2.capstone_count > 0 {
            write!(f, ", {} capstone{}\n\n", self.p2.capstone_count,
                if self.p2.capstone_count != 1 { "s" } else { "" }
            ).ok();
        } else {
            write!(f, "\n\n").ok();
        }

        for row in (0..board_size).rev() {
            write!(f, " {}   ", row + 1).ok();

            for column in 0..board_size {
                let mut c = String::new();
                write!(c, "[").ok();

                for (index, piece) in self.board[column][row].iter().rev().enumerate() {
                    if index > 0 {
                        write!(c, " ").ok();
                    }

                    write!(c, "{}", match piece.get_color() {
                        Color::White => "W",
                        Color::Black => "B",
                    }).ok();

                    match piece {
                        &Piece::StandingStone(_) => { write!(c, "S").ok(); },
                        &Piece::Capstone(_) => { write!(c, "C").ok(); },
                        _ => (),
                    }
                }

                write!(c, "]").ok();

                write!(f, "{:<width$}", c, width = column_widths[column]).ok();
            }

            write!(f, "\n").ok();
        }

        write!(f, "\n     ").ok();

        for (index, column_width) in column_widths.iter().enumerate() {
            write!(f, "{:<width$}", (index as u8 + 97) as char, width = column_width).ok();
        }

        write!(f, "\n")
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct StateSignature {
    pub next_color: Color,

    // The maps of the flatstones at each layer of the board for each player
    pub p1_flatstones: Vec<Bitmap>,
    pub p2_flatstones: Vec<Bitmap>,

    // The maps of all standing stones and capstones on the board
    pub standing_stones: Bitmap,
    pub capstones: Bitmap,

    // The map of all top pieces for each player
    pub p1_pieces: Bitmap,
    pub p2_pieces: Bitmap,
}
