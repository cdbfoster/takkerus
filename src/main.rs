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

#[macro_use]
extern crate lazy_static;
extern crate rand;
extern crate time;

mod ai;
mod tak;

use std::fmt::Write;

use tak::*;

fn main() {
    let mut state = State::new(5);

    let mut p1 = cli_player::CliPlayer::new();
    let mut p2 = ai::MinimaxBot::new(5);

    let mut ptn = String::new();
    'main: loop {
        println!("\n--------------------------------------------------");
        println!("{}", state);
        if state.ply_count >= 2 {
            println!("Previous turn:   {}\n", ptn);
        } else {
            println!("First turn\n");
        }

        'p1_move: loop {
            let ply = p1.get_move(&state);

            match state.execute_ply(&ply) {
                Ok(next) => {
                    state = next;

                    ptn = String::new();
                    write!(ptn, "{:<5} ", ply.to_ptn()).ok();

                    match state.check_win() {
                        Win::None => (),
                        _ => break 'main,
                    }

                    break 'p1_move;
                },
                Err(error) => println!("  {}", error),
            }
        }

        println!("\n--------------------------------------------------");
        println!("{}", state);
        println!("Previous move:   {}\n", ptn);

        'p2_move: loop {
            let ply = p2.get_move(&state);

            match state.execute_ply(&ply) {
                Ok(next) => {
                    state = next;

                    write!(ptn, "{}", ply.to_ptn()).ok();

                    match state.check_win() {
                        Win::None => (),
                        _ => break 'main,
                    }

                    break 'p2_move;
                },
                Err(error) => println!("  {}", error),
            }
        }
    }

    println!("\n--------------------------------------------------");
    println!("{}", state);
    println!("Final state:     {}\n", ptn);

    match state.check_win() {
        Win::Road(color) => match color {
            Color::White => println!("Player 1 wins! (R-0)"),
            Color::Black => println!("Player 2 wins! (0-R)"),
        },
        Win::Flat(color) => match color {
            Color::White => println!("Player 1 wins! (F-0)"),
            Color::Black => println!("Player 2 wins! (0-F)"),
        },
        Win::Draw => println!("Draw! (1/2-1/2)"),
        _ => (),
    }
}
