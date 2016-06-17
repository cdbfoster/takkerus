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

use std::i32;
use time;

use ai::{Ai, Extrapolatable};
use tak::{Color, Player, Ply, State, Win};

pub struct MinimaxBot {
    depth: u8,
}

impl MinimaxBot {
    pub fn new(depth: u8) -> MinimaxBot {
        MinimaxBot {
            depth: depth,
        }
    }
}

impl Ai for MinimaxBot {
    fn analyze(&mut self, state: &State) -> Vec<Ply> {
        minimax(state, Vec::new(), self.depth, i32::MIN + 1, i32::MAX).0
    }
}

impl Player for MinimaxBot {
    fn get_move(&mut self, state: &State) -> Ply {
        let old_time = time::precise_time_ns();

        let ply = self.analyze(state)[0].clone();

        let elapsed_time = time::precise_time_ns() - old_time;

        println!("[MinimaxBot] Decision time (depth {}): {:.3} seconds", self.depth, elapsed_time as f32 / 1000000000.0);

        ply
    }
}

type Eval = i32;

trait Evaluatable {
    fn evaluate(&self) -> Eval;
}

impl Evaluatable for State {
    fn evaluate(&self) -> Eval {
        match self.check_win() {
            Win::None => 0,
            Win::Road(win_color) |
            Win::Flat(win_color) => {
                let next_color = if self.ply_count % 2 == 0 {
                    Color::White
                } else {
                    Color::Black
                };

                if win_color == next_color {
                    i32::MAX - self.ply_count as i32
                } else {
                    i32::MIN + 1 + self.ply_count as i32
                }
            },
            Win::Draw => 0,
        }
    }
}

fn minimax(state: &State, mut move_set: Vec<Ply>, depth: u8, mut alpha: Eval, beta: Eval) -> (Vec<Ply>, Eval) {
    if depth == 0 || state.check_win() != Win::None {
        return (move_set, state.evaluate());
    }

    let mut best_next_move_set = Vec::new();
    let mut best_eval = i32::MIN;

    for ply in state.get_possible_moves() {
        let next_state = match state.execute_ply(&ply) {
            Ok(next) => next,
            Err(_) => continue,
        };

        let (mut next_move_set, mut next_eval) = minimax(&next_state, Vec::new(), depth - 1, -beta, -alpha);
        next_eval = -next_eval;

        if next_eval > best_eval {
            best_eval = next_eval;
            best_next_move_set = Vec::new();
            best_next_move_set.push(ply);
            best_next_move_set.append(&mut next_move_set);
        }

        if next_eval > alpha {
            alpha = next_eval;

            if alpha >= beta {
                break;
            }
        }
    }

    move_set.append(&mut best_next_move_set);

    (move_set, best_eval)
}
