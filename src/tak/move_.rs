//
// This file is part of tak-rs.
//
// tak-rs is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// tak-rs is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with tak-rs. If not, see <http://www.gnu.org/licenses/>.
//
// Copyright 2016 Chris Foster
//

use tak::{Color, Direction, Piece};

#[derive(Clone)]
pub struct Move {
    pub new_piece: Option<Piece>,
    pub grab: Option<usize>,
    pub x: usize,
    pub y: usize,
    pub direction: Option<Direction>,
    pub drop: Vec<usize>,
}

impl Move {
    pub fn new(new_piece: Option<Piece>, grab: Option<usize>, (x, y): (usize, usize), direction: Option<Direction>, drop: Vec<usize>) -> Move {
        Move {
            new_piece: new_piece,
            grab: grab,
            x: x,
            y: y,
            direction: direction,
            drop: drop,
        }
    }

    pub fn from_ptn(ptn: &str, color: Color) -> Option<Move> {
        let mut chars = ptn.chars();

        let mut next = chars.next();

        let mut new_piece = match next {
            Some('S') => {
                next = chars.next();
                Some(Piece::StandingStone(color))
            },
            Some('C') => {
                next = chars.next();
                Some(Piece::Capstone(color))
            },
            None => return None,
            _ => None,
        };

        let grab = match next {
            Some(c) => if c.is_digit(10) {
                next = chars.next();
                Some((c as u8 - 48) as usize)
            } else {
                None
            },
            None => return None,
        };

        let x = match next {
            Some(c) => if c.is_alphabetic() && c.is_lowercase() {
                (c as u8 - 97) as usize
            } else {
                return None;
            },
            None => return None,
        };

        let y = match chars.next() {
            Some(c) => if c.is_digit(10) {
                (c as u8 - 49) as usize
            } else {
                return None;
            },
            None => return None,
        };

        let direction = match chars.next() {
            Some('+') => Some(Direction::North),
            Some('>') => Some(Direction::East),
            Some('-') => Some(Direction::South),
            Some('<') => Some(Direction::West),
            None => {
                if new_piece.is_none() {
                    new_piece = Some(Piece::Flatstone(color));
                }
                None
            },
            _ => return None,
        };

        let mut drop = Vec::new();
        for c in chars {
            if c.is_digit(10) {
                drop.push((c as u8 - 48) as usize);
            } else {
                return None;
            }
        }

        Some(Move {
            new_piece: new_piece,
            grab: grab,
            x: x,
            y: y,
            direction: direction,
            drop: drop,
        })
    }
}
