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

use time;

use ai::{Ai, Extrapolatable};
use tak::{BitmapInterface, Color, Player, Ply, State, Win};

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
        let (plies, eval) = minimax(state, Vec::new(), self.depth, MIN_EVAL, MAX_EVAL);

        for i in 0..plies.len() {
            print!("{}: ", if (state.ply_count + i as u16) % 2 == 0 {
                "W"
            } else {
                "B"
            });
            println!("{:?}", plies[i]);
        }

        println!("{}", eval);
        plies
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

const MAX_EVAL: Eval = 100000;
const MIN_EVAL: Eval = -MAX_EVAL;

enum Weight {
    Flatstone =     400,
    StandingStone = 200,
    Capstone =      300,
}

trait Evaluatable {
    fn evaluate(&self) -> Eval;
}

impl Evaluatable for State {
    fn evaluate(&self) -> Eval {
        let next_color = if self.ply_count % 2 == 0 {
            Color::White
        } else {
            Color::Black
        };

        match self.check_win() {
            Win::None => (),
            Win::Road(win_color) |
            Win::Flat(win_color) => {
                if win_color == next_color {
                    return MAX_EVAL - self.ply_count as i32;
                } else {
                    return MIN_EVAL + self.ply_count as i32;
                }
            },
            Win::Draw => return 0,
        }

        let mut p1_eval = 0;
        let mut p2_eval = 0;

        let a = &self.analysis;

        p1_eval += (a.p1_pieces & !a.standing_stones & !a.capstones).get_population() as i32 * Weight::Flatstone as Eval;
        p2_eval += (a.p2_pieces & !a.standing_stones & !a.capstones).get_population() as i32 * Weight::Flatstone as Eval;

        p1_eval += (a.p1_pieces & a.standing_stones).get_population() as i32 * Weight::StandingStone as Eval;
        p2_eval += (a.p2_pieces & a.standing_stones).get_population() as i32 * Weight::StandingStone as Eval;

        p1_eval += (a.p1_pieces & a.capstones).get_population() as i32 * Weight::Capstone as Eval;
        p2_eval += (a.p2_pieces & a.capstones).get_population() as i32 * Weight::Capstone as Eval;

        match next_color {
            Color::White => p1_eval - p2_eval,
            Color::Black => p2_eval - p1_eval,
        }
    }
}

fn minimax(state: &State, mut move_set: Vec<Ply>, depth: u8, mut alpha: Eval, beta: Eval) -> (Vec<Ply>, Eval) {
    if depth == 0 || state.check_win() != Win::None {
        return (move_set, state.evaluate());
    }

    let mut best_next_move_set = Vec::new();
    let mut best_eval = MIN_EVAL;

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
