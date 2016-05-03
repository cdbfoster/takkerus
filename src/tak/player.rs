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

use tak::{Color, Piece};

pub struct Player {
    pub color: Color,
    pub pieces: Vec<Piece>,
}

impl Player {
    pub fn new(color: Color, flatstones: u8, capstones: u8) -> Player {
        let mut player = Player {
            color: color,
            pieces: Vec::new(),
        };

        for _ in 0..flatstones {
            player.pieces.push(Piece::Flatstone(color));
        }

        for _ in 0..capstones {
            player.pieces.push(Piece::Capstone(color));
        }

        player
    }

    pub fn has_flatstone(&self) -> bool {
        self.pieces.iter().any(|piece| *piece == Piece::Flatstone(self.color))
    }

    pub fn get_flatstone(&mut self) -> Option<Piece> {
        match self.pieces.iter().position(|piece| *piece == Piece::Flatstone(self.color)) {
            Some(index) => Some(self.pieces.remove(index)),
            None => None,
        }
    }

    pub fn has_capstone(&self) -> bool {
        self.pieces.iter().any(|piece| *piece == Piece::Capstone(self.color))
    }

    pub fn get_capstone(&mut self) -> Option<Piece> {
        match self.pieces.iter().position(|piece| *piece == Piece::Capstone(self.color)) {
            Some(index) => Some(self.pieces.remove(index)),
            None => None,
        }
    }
}
