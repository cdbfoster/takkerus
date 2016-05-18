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

use std::io::{self, Write};

use tak::{Color, Piece, Player, Ply, State};

pub struct CliPlayer {
    pub color: Color,
    stdin: io::Stdin,
}

impl CliPlayer {
    pub fn new(color: Color) -> CliPlayer {
        CliPlayer {
            color: color,
            stdin: io::stdin(),
        }
    }
}

impl Player for CliPlayer {
    fn get_move(&mut self, state: &State) -> Ply {
        let mut ply = None;

        while ply.is_none() {
            print!("Enter move ({}): ", match self.color {
                Color::White => "Player 1",
                Color::Black => "Player 2",
            });
            io::stdout().flush().ok();

            let mut input = String::new();
            match self.stdin.read_line(&mut input) {
                Ok(_) => ply = Ply::from_ptn(input.trim(), self.color),
                Err(e) => panic!("Error: {}", e),
            }

            if ply.is_none() {
                println!("Invalid PTN.");
            } else if state.ply_count < 2 {
                match ply {
                    Some(Ply::Place { piece: Piece::Flatstone(color), x, y }) => {
                        ply = Some(Ply::Place {
                            x: x,
                            y: y,
                            piece: Piece::Flatstone(color.flip()),
                        });
                    },
                    _ => {
                        println!("Illegal opening move.");
                        ply = None;
                    },
                }
            } else {
                match ply {
                    Some(Ply::Slide { x, y, .. }) => match state.board[x][y].last() {
                        Some(&Piece::Flatstone(color)) |
                        Some(&Piece::StandingStone(color)) |
                        Some(&Piece::Capstone(color)) => if color != self.color {
                            println!("Illegal move.");
                            ply = None;
                        },
                        _ => (),
                    },
                    _ => (),
                }
            }
        }

        ply.unwrap()
    }
}
