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

use std::fmt;

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Color {
    White,
    Black,
}

impl Color {
    pub fn flip(&self) -> Color {
        match *self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Piece {
    Flatstone(Color),
    StandingStone(Color),
    Capstone(Color),
}

impl Piece {
    pub fn get_color(&self) -> Color {
        match self {
            &Piece::Flatstone(color) |
            &Piece::StandingStone(color) |
            &Piece::Capstone(color) => color,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

impl Direction {
    pub fn to_offset(&self) -> (i8, i8) {
        match *self {
            Direction::North => (0, 1),
            Direction::East => (1, 0),
            Direction::South => (0, -1),
            Direction::West => (-1, 0),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Win {
    None,
    Road(Color),
    Flat(Color),
    Draw,
}

#[derive(Debug)]
pub enum GameError {
    IllegalPlacement,
    InsufficientPieces,
    IllegalSlide,
    OutOfBounds,
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &GameError::IllegalPlacement => write!(f, "Illegal placement."),
            &GameError::InsufficientPieces => write!(f, "Not enough pieces."),
            &GameError::IllegalSlide => write!(f, "Illegal slide."),
            &GameError::OutOfBounds => write!(f, "Out of bounds."),
        }
    }
}

pub use self::player::*;
pub use self::ply::Ply;
pub use self::state::State;
pub use self::state_analysis::{Bitmap, BitmapInterface, BOARD, EDGE, StateAnalysis};

pub mod player;
mod ply;
mod state;
pub mod state_analysis;
