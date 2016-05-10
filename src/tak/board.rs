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

use tak::{Color, Direction, GameError, Piece, Ply, TurnResult};

pub struct Board {
    pub spaces: Vec<Vec<Vec<Piece>>>,
    pub history: Vec<Ply>,
}

impl Board {
    pub fn new(size: usize) -> Board {
        Board {
            spaces: {
                let mut spaces = Vec::new();
                for _ in 0..size {
                    let mut column = Vec::new();
                    for _ in 0..size {
                        column.push(Vec::new());
                    }
                    spaces.push(column);
                }
                spaces
            },
            history: Vec::new(),
        }
    }

    pub fn execute(&mut self, next: Ply) -> Result<TurnResult, GameError> {
        let mut board = self.spaces.clone();
        let board_size = board.len();

        if next.x >= board_size || next.y >= board_size {
            return Err(GameError::OutOfBounds);
        }

        match next.new_piece {
            Some(ref new_piece) => {
                if board[next.x][next.y].is_empty() {
                    board[next.x][next.y].push(new_piece.clone());
                } else {
                    return Err(GameError::IllegalMove);
                }
            },
            None => if !board[next.x][next.y].is_empty() {
                fn slide_stack(stack: &mut Vec<Piece>, (x, y): (usize, usize), direction: Direction, drop: usize, board: &mut Vec<Vec<Vec<Piece>>>) -> Result<(usize, usize), GameError> {
                    let (target_x, target_y) = match direction {
                        Direction::North => if y == board.len() -1 {
                            return Err(GameError::OutOfBounds);
                        } else {
                            (x, y + 1)
                        },
                        Direction::East => if x == board.len() - 1 {
                            return Err(GameError::OutOfBounds);
                        } else {
                            (x + 1, y)
                        },
                        Direction::South => if y == 0 {
                            return Err(GameError::OutOfBounds);
                        } else {
                            (x, y - 1)
                        },
                        Direction::West => if x == 0 {
                            return Err(GameError::OutOfBounds);
                        } else {
                            (x - 1, y)
                        },
                    };

                    let mut dropped = Vec::new();
                    for _ in 0..drop {
                        dropped.push(stack.remove(0));
                    }

                    if board[target_x][target_y].len() > 0 {
                        let target_top = board[target_x][target_y].last().unwrap().clone();
                        match target_top {
                            Piece::Capstone(_) => return Err(GameError::IllegalMove),
                            Piece::StandingStone(color) => if dropped.len() == 1 {
                                match *dropped.first().unwrap() {
                                    Piece::Capstone(_) => *board[target_x][target_y].last_mut().unwrap() = Piece::Flatstone(color),
                                    _ => return Err(GameError::IllegalMove),
                                }
                            } else {
                                return Err(GameError::IllegalMove);
                            },
                            _ => (),
                        }
                    }

                    for _ in 0..dropped.len() {
                        board[target_x][target_y].push(dropped.remove(0));
                    }

                    Ok((target_x, target_y))
                }

                let drop = if board[next.x][next.y].len() > 1 {
                    match next.grab {
                        Some(grab) => if grab == 0 || grab > board_size || grab > board[next.x][next.y].len() {
                            return Err(GameError::InvalidMove);
                        } else {
                            board[next.x][next.y].len() - grab
                        },
                        None => return Err(GameError::InvalidMove),
                    }
                } else {
                    if next.grab.is_some() || !next.drop.is_empty() {
                        return Err(GameError::InvalidMove);
                    }

                    0
                };

                let mut stack = Vec::new();
                for _ in drop..board[next.x][next.y].len() {
                    stack.push(board[next.x][next.y].remove(drop));
                }

                if next.drop.is_empty() {
                    let stack_len = stack.len();
                    match next.direction {
                        Some(direction) => match slide_stack(&mut stack, (next.x, next.y), direction, stack_len, &mut board) {
                            Ok(_) => (),
                            Err(error) => return Err(error),
                        },
                        None => return Err(GameError::InvalidMove),
                    }
                } else {
                    let (mut x, mut y) = (next.x, next.y);
                    let direction = match next.direction {
                        Some(direction) => direction,
                        None => return Err(GameError::InvalidMove),
                    };
                    for drop in next.drop.iter() {
                        if *drop > stack.len() {
                            return Err(GameError::InvalidMove);
                        }

                        match slide_stack(&mut stack, (x, y), direction, *drop, &mut board) {
                            Ok((target_x, target_y)) => {
                                x = target_x;
                                y = target_y;
                            },
                            Err(error) => return Err(error),
                        }
                    }
                }

                if !stack.is_empty() {
                    return Err(GameError::InvalidMove);
                }
            } else {
                return Err(GameError::InvalidMove);
            },
        }

        self.spaces = board;

        match self.check_win() {
            Some(color) => Ok(TurnResult::Win(color)),
            None => Ok(TurnResult::Normal),
        }
    }

    pub fn check_win(&self) -> Option<Color> {
        fn check_neighbors(board: &Vec<Vec<Vec<Piece>>>, (x, y): (usize, usize), (old_x, old_y): (usize, usize), direction: Direction, color: Color) -> Option<Color> {
            fn contributes(board: &Vec<Vec<Vec<Piece>>>, (x, y): (usize, usize), color: Color) -> bool {
                match board[x][y].last() {
                    Some(&Piece::StandingStone(_)) => false,
                    Some(&Piece::Flatstone(c)) |
                    Some(&Piece::Capstone(c)) => if c == color {
                        true
                    } else {
                        false
                    },
                    None => false,
                }
            }

            match direction {
                Direction::North => {
                    if y + 1 < board.len() {
                        if contributes(board, (x, y + 1), color) {
                            if y + 1 == board.len() - 1 {
                                return Some(color);
                            } else {
                                let result = check_neighbors(board, (x, y + 1), (x, y), direction, color);
                                if result.is_some() {
                                    return result;
                                }
                            }
                        }
                    }

                    if x > 0 && x - 1 != old_x {
                        if contributes(board, (x - 1, y), color) {
                            let result = check_neighbors(board, (x - 1, y), (x, y), direction, color);
                            if result.is_some() {
                                return result;
                            }
                        }
                    }

                    if x + 1 < board.len() && x + 1 != old_x {
                        if contributes(board, (x + 1, y), color) {
                            let result = check_neighbors(board, (x + 1, y), (x, y), direction, color);
                            if result.is_some() {
                                return result;
                            }
                        }
                    }
                },
                Direction::East => {
                    if x + 1 < board.len() {
                        if contributes(board, (x + 1, y), color) {
                            if x + 1 == board.len() - 1 {
                                return Some(color);
                            } else {
                                let result = check_neighbors(board, (x + 1, y), (x, y), direction, color);
                                if result.is_some() {
                                    return result;
                                }
                            }
                        }
                    }

                    if y + 1 < board.len() && y + 1 != old_y {
                        if contributes(board, (x, y + 1), color) {
                            let result = check_neighbors(board, (x, y + 1), (x, y), direction, color);
                            if result.is_some() {
                                return result;
                            }
                        }
                    }

                    if y > 0 && y - 1 != old_y {
                        if contributes(board, (x, y - 1), color) {
                            let result = check_neighbors(board, (x, y - 1), (x, y), direction, color);
                            if result.is_some() {
                                return result;
                            }
                        }
                    }
                },
                _ => panic!("A direction other than North and East was passed to check_neighbors"),
            }

            None
        }

        for x in 0..self.spaces.len() {
            let color = match self.spaces[x][0].last() {
                Some(&Piece::StandingStone(_)) => continue,
                Some(&Piece::Flatstone(color)) |
                Some(&Piece::Capstone(color)) => color,
                None => continue,
            };
            let result = check_neighbors(&self.spaces, (x, 0), (x, 0), Direction::North, color);
            if result.is_some() {
                return result;
            }
        }

        for y in 0..self.spaces.len() {
            let color = match self.spaces[0][y].last() {
                Some(&Piece::StandingStone(_)) => continue,
                Some(&Piece::Flatstone(color)) |
                Some(&Piece::Capstone(color)) => color,
                None => continue,
            };
            let result = check_neighbors(&self.spaces, (0, y), (0, y), Direction::East, color);
            if result.is_some() {
                return result;
            }
        }

        if !self.spaces.iter().any(|column| column.iter().any(|stack| stack.is_empty())) {
            let white_count = self.spaces.iter().fold(0,
                |sum, column| sum + column.iter().fold(0,
                    |sum, stack| sum + match stack.last() {
                        Some(&Piece::Flatstone(Color::White)) => 1,
                        _ => 0,
                    }
                )
            );

            let black_count = self.spaces.iter().fold(0,
                |sum, column| sum + column.iter().fold(0,
                    |sum, stack| sum + match stack.last() {
                        Some(&Piece::Flatstone(Color::Black)) => 1,
                        _ => 0,
                    }
                )
            );

            if white_count > black_count {
                Some(Color::White)
            } else if black_count > white_count {
                Some(Color::Black)
            } else {
                None
            }
        } else {
            None
        }
    }
}
