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

use tak::{Color, Piece};

lazy_static! {
    pub static ref EDGE: [[Bitmap; 4]; 9] = generate_edge_masks();
    pub static ref BOARD: [Bitmap; 9] = generate_board_masks();
}

pub type Bitmap = u64;

#[derive(Clone, Debug)]
pub struct StateAnalysis {
    pub board_size: usize,

    // The number of flatstones on the board for each player
    pub p1_flatstone_count: u8,
    pub p2_flatstone_count: u8,

    // The maps of the flatstones at each layer of the board for each player
    pub p1_flatstones: Vec<Bitmap>,
    pub p2_flatstones: Vec<Bitmap>,

    // The maps of all standing stones and capstones on the board
    pub standing_stones: Bitmap,
    pub capstones: Bitmap,

    // The map of all top pieces for each player
    pub p1_pieces: Bitmap,
    pub p2_pieces: Bitmap,

    // The maps of each discrete island of road-contributing pieces for each player
    pub p1_road_groups: Vec<Bitmap>,
    pub p2_road_groups: Vec<Bitmap>,
}

impl StateAnalysis {
    pub fn new(board_size: usize) -> StateAnalysis {
        StateAnalysis {
            board_size: board_size,
            p1_flatstone_count: 0,
            p2_flatstone_count: 0,
            p1_flatstones: Vec::new(),
            p2_flatstones: Vec::new(),
            standing_stones: 0x0000000000000000,
            capstones: 0x0000000000000000,
            p1_pieces: 0x0000000000000000,
            p2_pieces: 0x0000000000000000,
            p1_road_groups: Vec::new(),
            p2_road_groups: Vec::new(),
        }
    }

    pub fn add_flatstone(&mut self, color: Color, x: usize, y: usize, z: usize) {
        match color {
            Color::White => {
                self.p1_flatstone_count += 1;

                if z >= self.p1_flatstones.len() {
                    for _ in 0..(z - self.p1_flatstones.len() + 1) {
                        self.p1_flatstones.push(0x0000000000000000);
                    }
                }

                self.p1_flatstones[z].set(x, y, self.board_size);
                self.p1_pieces.set(x, y, self.board_size);
            },
            Color::Black => {
                self.p2_flatstone_count += 1;

                if z >= self.p2_flatstones.len() {
                    for _ in 0..(z - self.p2_flatstones.len() + 1) {
                        self.p2_flatstones.push(0x0000000000000000);
                    }
                }

                self.p2_flatstones[z].set(x, y, self.board_size);
                self.p2_pieces.set(x, y, self.board_size);
            },
        }
    }

    pub fn remove_flatstone(&mut self, color: Color, x: usize, y: usize, z: usize) {
        match color {
            Color::White => {
                self.p1_flatstone_count -= 1;
                self.p1_flatstones[z].clear(x, y, self.board_size);
                self.p1_pieces.clear(x, y, self.board_size);
            },
            Color::Black => {
                self.p2_flatstone_count -= 1;
                self.p2_flatstones[z].clear(x, y, self.board_size);
                self.p2_pieces.clear(x, y, self.board_size);
            },
        }
    }

    pub fn reveal_flatstone(&mut self, color: Color, x: usize, y: usize) {
        match color {
            Color::White => {
                self.p1_flatstone_count += 1;
                self.p1_pieces.set(x, y, self.board_size);
            },
            Color::Black => {
                self.p2_flatstone_count += 1;
                self.p2_pieces.set(x, y, self.board_size);
            },
        }
    }

    pub fn cover_flatstone(&mut self, color: Color, x: usize, y: usize) {
        match color {
            Color::White => {
                self.p1_flatstone_count -= 1;
                self.p1_pieces.clear(x, y, self.board_size);
            },
            Color::Black => {
                self.p2_flatstone_count -= 1;
                self.p2_pieces.clear(x, y, self.board_size);
            },
        }
    }

    pub fn add_blocking_stone(&mut self, piece: &Piece, x: usize, y: usize) {
        match piece {
            &Piece::StandingStone(color) => if color == Color::White {
                self.standing_stones.set(x, y, self.board_size);
                self.p1_pieces.set(x, y, self.board_size);
            } else {
                self.standing_stones.set(x, y, self.board_size);
                self.p2_pieces.set(x, y, self.board_size);
            },
            &Piece::Capstone(color) => if color == Color::White {
                self.capstones.set(x, y, self.board_size);
                self.p1_pieces.set(x, y, self.board_size);
            } else {
                self.capstones.set(x, y, self.board_size);
                self.p2_pieces.set(x, y, self.board_size);
            },
            _ => panic!("StateAnalysis.add_blocking_stone was passed a flatstone!"),
        }
    }

    pub fn remove_blocking_stone(&mut self, piece: &Piece, x: usize, y: usize) {
        match piece {
            &Piece::StandingStone(color) => if color == Color::White {
                self.standing_stones.clear(x, y, self.board_size);
                self.p1_pieces.clear(x, y, self.board_size);
            } else {
                self.standing_stones.clear(x, y, self.board_size);
                self.p2_pieces.clear(x, y, self.board_size);
            },
            &Piece::Capstone(color) => if color == Color::White {
                self.capstones.clear(x, y, self.board_size);
                self.p1_pieces.clear(x, y, self.board_size);
            } else {
                self.capstones.clear(x, y, self.board_size);
                self.p2_pieces.clear(x, y, self.board_size);
            },
            _ => panic!("StateAnalysis.remove_blocking_stone was passed a flatstone!"),
        }
    }

    pub fn calculate_road_groups(&mut self) {
        self.p1_road_groups = (self.p1_pieces & !self.standing_stones).get_groups(self.board_size);
        self.p2_road_groups = (self.p2_pieces & !self.standing_stones).get_groups(self.board_size);
    }
}

pub trait BitmapInterface {
    fn set(&mut self, x: usize, y: usize, stride: usize);
    fn clear(&mut self, x: usize, y: usize, stride: usize);
    fn get(&self, x: usize, y: usize, stride: usize) -> bool;
    fn get_groups(&self, stride: usize) -> Vec<Bitmap>;
}

impl BitmapInterface for Bitmap {
    fn set(&mut self, x: usize, y: usize, stride: usize) {
        *self |= 1 << ((stride - 1 - x) + y * stride);
    }

    fn clear(&mut self, x: usize, y: usize, stride: usize) {
        *self &= !(1 << ((stride - 1 - x) + y * stride));
    }

    fn get(&self, x: usize, y: usize, stride: usize) -> bool {
        (self >> ((stride - 1 - x) + y * stride)) & 1 == 1
    }

    fn get_groups(&self, stride: usize) -> Vec<Bitmap> {
        fn pop_bit(map: Bitmap) -> (Bitmap, Bitmap) {
            let remainder = map & (map - 1);
            let bit = map & !remainder;
            (bit, remainder)
        }

        fn flood(bit: Bitmap, bounds: Bitmap, stride: usize) -> Bitmap {
            use tak::Direction::*;
            let mut total = bit;

	        loop {
	            let mut next = total;

	            next |= (total << 1) & !EDGE[stride][East as usize];
	            next |= (total >> 1) & !EDGE[stride][West as usize];
	            next |= total << stride;
	            next |= total >> stride;
	            next &= bounds;

	            if next == total {
	                break;
                }

                total = next;
            }

            total
        }

        if *self == 0 {
            return Vec::new();
        }

        let mut groups = Vec::new();
        let mut map = *self;

        loop {
            let (bit, mut remainder) = pop_bit(map);

            let group = flood(bit, map, stride);
            groups.push(group);

            remainder &= !group;
            if remainder == 0 {
                break;
            }

            map = remainder;
        }

        groups
    }
}

fn generate_edge_masks() -> [[Bitmap; 4]; 9] {
    use tak::Direction::*;

    let mut edge = [[0; 4]; 9];

    for size in 3..(8 + 1) {
        for y in 0..size {
            edge[size][East as usize] |= 1 << (y * size);
        }
        edge[size][West as usize] = edge[size][East as usize] << (size - 1);

        edge[size][South as usize] = (1 << size) - 1;
        edge[size][North as usize] = edge[size][South as usize] << (size * (size - 1));
    }

    edge
}

fn generate_board_masks() -> [Bitmap; 9] {
    let mut board = [0; 9];

    for size in 3..(8 + 1) {
        board[size] = 0xFFFFFFFFFFFFFFFF >> (64 - size * size);
    }

    board
}

