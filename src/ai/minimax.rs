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

use std::cell::RefCell;
use std::fmt::{self, Write};

use rand::{thread_rng, Rng};
use time;

use ai::{Ai, Extrapolatable};
use tak::{Bitmap, BitmapInterface, BOARD, Color, EDGE, Player, Ply, State, Win};

pub struct MinimaxBot {
    depth: u8,
    stats: Vec<RefCell<Statistics>>,
}

impl MinimaxBot {
    pub fn new(depth: u8) -> MinimaxBot {
        MinimaxBot {
            depth: depth,
            stats: Vec::new(),
        }
    }

    fn minimax(&self, state: &State, principal_variation: &mut Vec<Ply>, depth: u8, mut alpha: Eval, beta: Eval) -> Eval {
        if depth == 0 || state.check_win() != Win::None {
            self.stats.last().unwrap().borrow_mut().evaluated += 1;

            principal_variation.clear();
            return state.evaluate();
        }

        self.stats.last().unwrap().borrow_mut().visited += 1;

        let ply_generator = PlyGenerator::new(
            state,
            match principal_variation.first() {
                Some(ply) => Some(ply.clone()),
                None => None,
            },
        );

        let mut next_principal_variation = Vec::new();
        let mut first_iteration = true;

        for ply in ply_generator {
            let next_state = match state.execute_ply(&ply) {
                Ok(next) => next,
                Err(_) => continue,
            };

            let next_eval = if first_iteration {
                -self.minimax(
                    &next_state, &mut next_principal_variation, depth - 1,
                    -beta, -alpha,
                )
            } else {
                let next_eval = -self.minimax(
                    &next_state, &mut next_principal_variation, depth - 1,
                    -alpha - 1, -alpha,
                );

                if next_eval > alpha && next_eval < beta {
                    -self.minimax(
                        &next_state, &mut next_principal_variation, depth - 1,
                        -beta, -alpha,
                    )
                } else {
                    next_eval
                }
            };

            if next_eval > alpha {
                alpha = next_eval;

                principal_variation.clear();
                principal_variation.push(ply);
                principal_variation.append(&mut next_principal_variation.clone());

                if alpha >= beta {
                    return beta;
                }
            }

            first_iteration = false;
        }

        alpha
    }
}

impl Ai for MinimaxBot {
    fn analyze(&mut self, state: &State) -> Vec<Ply> {
        let mut principal_variation = Vec::new();

        for depth in 1..self.depth + 1 {
            self.stats.push(RefCell::new(Statistics::new(depth)));

            let eval = self.minimax(state, &mut principal_variation, depth, MIN_EVAL, MAX_EVAL);

            if eval.abs() > WIN_THRESHOLD {
                break;
            }
        }

        principal_variation
    }

    fn get_stats(&self) -> Box<fmt::Display> {
        struct StatisticPrinter(Vec<Statistics>);

        impl fmt::Display for StatisticPrinter {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let result = write!(f, "Minimax Statistics:");
                for stats in self.0.iter() {
                    write!(f, "\n  Depth {}:\n", stats.depth).ok();
                    write!(f, "    {:10} {:8}\n", "Visited:", stats.visited).ok();
                    write!(f, "    {:10} {:8}", "Evaluated:", stats.evaluated).ok();
                }
                result
            }
        }

        Box::new(StatisticPrinter(
            self.stats.iter().map(|stats| stats.borrow().clone()).collect::<Vec<Statistics>>()
        ))
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

    fn get_name(&self) -> String {
        let mut name = String::new();
        write!(name, "Takkerus v{} (MinimaxBot - Depth: {})",
            option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"),
            self.depth,
        ).ok();

        name
    }
}

#[derive(Clone)]
pub struct Statistics {
    depth: u8,
    visited: u32,
    evaluated: u32,
}

impl Statistics {
    pub fn new(depth: u8) -> Statistics {
        Statistics {
            depth: depth,
            visited: 0,
            evaluated: 0,
        }
    }
}

struct PlyGenerator<'a> {
    state: &'a State,
    principal_ply: Option<Ply>,
    plies: Vec<Ply>,
    operation: u8,
}

impl<'a> PlyGenerator<'a> {
    fn new(state: &'a State, principal_ply: Option<Ply>) -> PlyGenerator<'a> {
        PlyGenerator {
            state: state,
            principal_ply: principal_ply,
            plies: Vec::new(),
            operation: 0,
        }
    }
}

impl<'a> Iterator for PlyGenerator<'a> {
    type Item = Ply;

    fn next(&mut self) -> Option<Ply> {
        loop {
            if self.operation == 0 {
                self.operation += 1;

                if self.principal_ply.is_some() {
                    return self.principal_ply.clone();
                }
            }

            if self.operation == 1 {
                self.operation += 1;

                self.plies = self.state.get_possible_plies();
                thread_rng().shuffle(self.plies.as_mut_slice());
            }

            if self.operation == 2 {
                let ply = self.plies.pop();

                if ply != self.principal_ply || ply.is_none() {
                    return ply;
                }
            }
        }
    }
}

pub type Eval = i32;

const MAX_EVAL: Eval = 100000;
const MIN_EVAL: Eval = -MAX_EVAL;

const WIN_THRESHOLD: Eval = 99000;

struct Weights {
    flatstone: Eval,
    standing_stone: Eval,
    capstone: Eval,

    hard_flat: Eval,
    soft_flat: Eval,

    threat: Eval,

    group: [Eval; 8],
}

const WEIGHT: Weights = Weights {
    flatstone:          400,
    standing_stone:     200,
    capstone:           300,

    hard_flat:          125,
    soft_flat:          -75,

    threat:             200,

    group: [0, 0, 100, 200, 400, 600, 0, 0],
};

pub trait Evaluatable {
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

        p1_eval += a.p1_flatstone_count as i32 * WEIGHT.flatstone as Eval;
        p2_eval += a.p2_flatstone_count as i32 * WEIGHT.flatstone as Eval;

        p1_eval += (a.p1_pieces & a.standing_stones).get_population() as i32 * WEIGHT.standing_stone as Eval;
        p2_eval += (a.p2_pieces & a.standing_stones).get_population() as i32 * WEIGHT.standing_stone as Eval;

        p1_eval += (a.p1_pieces & a.capstones).get_population() as i32 * WEIGHT.capstone as Eval;
        p2_eval += (a.p2_pieces & a.capstones).get_population() as i32 * WEIGHT.capstone as Eval;

        // Stacked flatstones
        let mut p1_hard_flats = -(a.p1_flatstone_count as i32); // Top-level flatstones don't count
        let mut p1_soft_flats = 0;
        for level in a.p1_flatstones.iter() {
            if *level != 0 {
                p1_hard_flats += (level & a.p1_pieces).get_population() as i32;
                p1_soft_flats += (level & a.p2_pieces).get_population() as i32;
            }
        }

        let mut p2_hard_flats = -(a.p2_flatstone_count as i32);
        let mut p2_soft_flats = 0;
        for level in a.p2_flatstones.iter() {
            if *level != 0 {
                p2_hard_flats += (level & a.p2_pieces).get_population() as i32;
                p2_soft_flats += (level & a.p1_pieces).get_population() as i32;
            }
        }

        p1_eval += p1_hard_flats * WEIGHT.hard_flat as Eval + p2_soft_flats * WEIGHT.soft_flat as Eval;
        p2_eval += p2_hard_flats * WEIGHT.hard_flat as Eval + p1_soft_flats * WEIGHT.soft_flat as Eval;

        // Road groups
        let evaluate_groups = |groups: &Vec<Bitmap>| {
            let mut eval = 0;

            for group in groups.iter() {
                let (width, height) = group.get_dimensions(a.board_size);

                eval += WEIGHT.group[width] + WEIGHT.group[height];
            }

            eval
        };

        p1_eval += evaluate_groups(&a.p1_road_groups);
        p2_eval += evaluate_groups(&a.p2_road_groups);

        // Threats
        let total_pieces = a.p1_pieces | a.p2_pieces;

        let evaluate_threats = |groups: &Vec<Bitmap>| {
            let mut expanded_groups = vec![0; groups.len()];
            let mut threats = 0;

            for i in 0..groups.len() {
                expanded_groups[i] = groups[i].grow(BOARD[a.board_size], a.board_size);
            }

            let is_road = |group: Bitmap| {
                use tak::Direction::*;

                if (group & EDGE[a.board_size][North as usize] != 0 &&
                    group & EDGE[a.board_size][South as usize] != 0) ||
                   (group & EDGE[a.board_size][West as usize] != 0 &&
                    group & EDGE[a.board_size][East as usize] != 0) {
                    return true;
                }

                false
            };

            for l in 0..groups.len() {
                for r in l..groups.len() {
                    if l != r {
                        let overlap = expanded_groups[l] & expanded_groups[r] & !total_pieces;

                        if overlap == 0 {
                            continue;
                        }

                        if is_road(groups[l] | groups[r] | overlap) {
                            threats += 1;
                        }
                    }
                }
            }

            threats * WEIGHT.threat
        };

        p1_eval += evaluate_threats(&a.p1_road_groups);
        p2_eval += evaluate_threats(&a.p2_road_groups);

        match next_color {
            Color::White => p1_eval - p2_eval,
            Color::Black => p2_eval - p1_eval,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32;
    use time;

    use ai::Ai;
    use tak::*;
    use super::{MinimaxBot, Evaluatable};

    #[test]
    fn test_minimax() {
        let games = 50;
        let mut total_ply_count = 0;
        let mut total_min_time = f32::MAX;
        let mut total_max_time = 0.0;
        let mut total_time = 0.0;

        let mut p1_wins = 0;
        let mut p2_wins = 0;

        for _ in 1..(games + 1) {
            let mut state = State::new(5);

            let depth = 5;

            let mut p1 = MinimaxBot::new(depth);
            let mut p1_min_time = f32::MAX;
            let mut p1_max_time = 0.0;
            let mut p1_total_time = 0.0;

            let mut p2 = MinimaxBot::new(depth);
            let mut p2_min_time = f32::MAX;
            let mut p2_max_time = 0.0;
            let mut p2_total_time = 0.0;

            let mut ply_count = 0;

            'game: loop {
                let old_time = time::precise_time_ns();

                let plies = if ply_count % 2 == 0 {
                    p1.analyze(&state)
                } else {
                    p2.analyze(&state)
                };

                let eval = {
                    let mut temp_state = state.clone();
                    for ply in plies.iter() {
                        match temp_state.execute_ply(ply) {
                            Ok(next) => temp_state = next,
                            Err(error) => panic!("Error calculating evaluation: {}", error),
                        }
                    }
                    temp_state.evaluate() * -((plies.len() as i32 % 2) * 2 - 1)
                };

                let elapsed_time = (time::precise_time_ns() - old_time) as f32 / 1000000000.0;

                if ply_count % 2 == 0 {
                    if elapsed_time < p1_min_time {
                        p1_min_time = elapsed_time;
                    }

                    if elapsed_time > p1_max_time {
                        p1_max_time = elapsed_time;
                    }

                    p1_total_time += elapsed_time;
                } else {
                    if elapsed_time < p2_min_time {
                        p2_min_time = elapsed_time;
                    }

                    if elapsed_time > p2_max_time {
                        p2_max_time = elapsed_time;
                    }

                    p2_total_time += elapsed_time;
                }

                ply_count += 1;

                match state.execute_ply(&plies[0]) {
                    Ok(next) => {
                        state = next;

                        match state.check_win() {
                            Win::None => (),
                            _ => break 'game,
                        }
                    },
                    Err(error) => panic!("Minimax returned an illegal move.\n--------------------------------------------------\n{}\n{:?}\nError: {}", state, plies[0], error),
                }

                if ply_count % 10 == 0 {
                    println!("--------------------------------------------------");
                    println!("{}", state);
                    println!("{:?}\n", state.analysis);
                }

                println!("{:2}: {} {:6} {:7.3} {:7.3} {:8.3} {}", ply_count,
                    if ply_count % 2 == 1 {
                        "White"
                    } else {
                        "Black"
                    },
                    eval,
                    elapsed_time,
                    if ply_count % 2 == 1 {
                        p1_total_time / ((ply_count + 1) / 2) as f32
                    } else {
                        p2_total_time / ((ply_count + 1) / 2) as f32
                    },
                    if ply_count % 2 == 1 {
                        p1_total_time
                    } else {
                        p2_total_time
                    },
                    &plies[0].to_ptn(),
                );
            }

            match state.check_win() {
                Win::Road(color) => match color {
                    Color::White => println!("White wins. (R-0)"),
                    Color::Black => println!("Black wins. (0-R)"),
                },
                Win::Flat(color) => match color {
                    Color::White => println!("White wins. (F-0)"),
                    Color::Black => println!("Black wins. (0-F)"),
                },
                Win::Draw => println!("Draw. (1/2-1/2)"),
                _ => (),
            }

            match state.check_win() {
                Win::Road(color) |
                Win::Flat(color) => match color {
                    Color::White => p1_wins += 1,
                    Color::Black => p2_wins += 1,
                },
                _ => (),
            }

            println!("--------------------------------------------------");
            println!("{}", state);
            println!("{:?\n}", state.analysis);

            println!("Plies: {}", ply_count);
            total_ply_count += ply_count;

            println!("Minimum ply time: {:.3}", if p1_min_time < p2_min_time {
                total_min_time = if p1_min_time < total_min_time {
                    p1_min_time
                } else {
                    total_min_time
                };
                p1_min_time
            } else {
                total_min_time = if p2_min_time < total_min_time {
                    p2_min_time
                } else {
                    total_min_time
                };
                p2_min_time
            });
            println!("Maximum ply time: {:.3}", if p1_max_time > p2_max_time {
                total_max_time = if p1_max_time > total_max_time {
                    p1_max_time
                } else {
                    total_max_time
                };
                p1_max_time
            } else {
                total_max_time = if p2_max_time > total_max_time {
                    p2_max_time
                } else {
                    total_max_time
                };
                p2_max_time
            });

            println!("Average ply time: {:.3}", (p1_total_time + p2_total_time) / ply_count as f32);

            println!("Game time: {:.3}", p1_total_time + p2_total_time);
            total_time += p1_total_time + p2_total_time;

            println!("\nWhite wins: {} / {}", p1_wins, games);
            println!("Black wins: {} / {}\n", p2_wins, games);
        }

        println!("Games: {}", games);
        println!("Total plies: {}", total_ply_count);
        println!("Absolute minimum ply time: {:.5}", total_min_time);
        println!("Absolute maximum ply time: {:.3}", total_max_time);
        println!("Average ply time: {:.3}", total_time / total_ply_count as f32);
        println!("Average plies per game: {:.1}", total_ply_count as f32 / games as f32);
        println!("Average game time: {:.3}", total_time / games as f32);
    }
}
