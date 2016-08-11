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

use std::any::Any;
use std::cell::RefCell;
use std::cmp;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::hash::BuildHasherDefault;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use fnv::FnvHasher;
use rand::{Rng, SeedableRng, StdRng};
use time;

use ai::{Ai, Extrapolatable};
use tak::{Bitmap, BitmapInterface, BOARD, Color, EDGE, Message, Player, Ply, State, StateSignature, Win};

lazy_static! {
    pub static ref RNG: Mutex<StdRng> = Mutex::new(StdRng::from_seed(&[time::precise_time_ns() as usize]));
}

pub struct Minimax {
    depth: u8,
    goal: u16,
    history: RefCell<BTreeMap<u64, u32>>,
    transposition_table: RefCell<HashMap<StateSignature, TranspositionTableEntry, BuildHasherDefault<FnvHasher>>>,
    stats: Vec<RefCell<Statistics>>,

    cancel: Arc<Mutex<bool>>,

    states: Vec<RefCell<State>>,
}

impl Minimax {
    pub fn new(depth: u8, goal: u16) -> Minimax {
        let max_depth = if depth == 0 {
            15
        } else {
            depth as usize
        };

        Minimax {
            depth: depth,
            goal: goal,
            history: RefCell::new(BTreeMap::new()),
            transposition_table: RefCell::new(HashMap::default()),
            stats: Vec::new(),
            cancel: Arc::new(Mutex::new(false)),
            states: vec![RefCell::new(State::new(5)); max_depth],
        }
    }

    fn minimax(&self, state: &State, principal_variation: &mut Vec<Ply>, depth: u8, max_depth: u8, mut alpha: Eval, beta: Eval) -> Eval {
        if depth == 0 || state.check_win() != Win::None {
            self.stats.last().unwrap().borrow_mut().evaluated += 1;

            principal_variation.clear();
            return state.evaluate();
        }

        let search_iteration = (max_depth - depth) as usize;

        self.stats.last().unwrap().borrow_mut().visited += 1;

        match self.transposition_table.borrow().get(&state.get_signature()) {
            Some(entry) => {
                self.stats.last().unwrap().borrow_mut().tt_hits += 1;

                let mut usable = false;

                if entry.depth >= depth &&
                  (entry.bound_type == BoundType::Exact ||
                  (entry.bound_type == BoundType::Upper && entry.value < alpha) ||
                  (entry.bound_type == BoundType::Lower && entry.value >= beta)) {
                    usable = true;
                }

                if entry.bound_type == BoundType::Exact && entry.value.abs() > WIN_THRESHOLD {
                    usable = true;
                }

                if usable {
                    match state.execute_ply_preallocated(&entry.principal_variation[0], &mut *self.states.get(search_iteration).unwrap().borrow_mut()) {
                        Ok(_) => {
                            self.stats.last().unwrap().borrow_mut().tt_saves += 1;

                            principal_variation.clear();
                            principal_variation.append(&mut entry.principal_variation.clone());

                            return entry.value;
                        },
                        _ => (),
                    }
                }
            },
            None => (),
        }

        let ply_generator = PlyGenerator::new(
            self,
            state,
            match principal_variation.first() {
                Some(ply) => Some(ply.clone()),
                None => None,
            },
        );

        let mut next_principal_variation = if !principal_variation.is_empty() {
            principal_variation.clone()[1..].to_vec()
        } else {
            Vec::new()
        };

        let mut first_iteration = true;
        let mut raised_alpha = false;

        for ply in ply_generator {
            let next_state = {
                match state.execute_ply_preallocated(&ply, &mut *self.states.get(search_iteration).unwrap().borrow_mut()) {
                    Err(_) => continue,
                    _ => (),
                };

                self.states[search_iteration].borrow()
            };

            let next_eval = if first_iteration {
                -self.minimax(
                    &next_state, &mut next_principal_variation, depth - 1, max_depth,
                    -beta, -alpha,
                )
            } else {
                let mut npv = next_principal_variation.clone();
                let next_eval = -self.minimax(
                    &next_state, &mut npv, depth - 1, max_depth,
                    -alpha - 1, -alpha,
                );

                if next_eval > alpha && next_eval < beta {
                    -self.minimax(
                        &next_state, &mut next_principal_variation, depth - 1, max_depth,
                        -beta, -alpha,
                    )
                } else {
                    next_principal_variation = npv;
                    next_eval
                }
            };

            if next_eval > alpha {
                alpha = next_eval;
                raised_alpha = true;

                principal_variation.clear();
                principal_variation.push(ply.clone());
                principal_variation.append(&mut next_principal_variation.clone());

                if alpha >= beta {
                    {
                        let mut history = self.history.borrow_mut();
                        let entry = history.entry(ply.hash()).or_insert(0);
                        *entry += 1 << depth;
                    }
                    break;
                }
            }

            first_iteration = false;

            if *self.cancel.lock().unwrap() == true {
                return 0;
            }
        }

        match principal_variation.first() {
            Some(ply) => match state.execute_ply_preallocated(ply, &mut *self.states.get(search_iteration).unwrap().borrow_mut()) {
                Ok(_) => {
                    self.transposition_table.borrow_mut().insert(state.get_signature(),
                        TranspositionTableEntry {
                            depth: depth,
                            value: alpha,
                            bound_type: if !raised_alpha {
                                BoundType::Upper
                            } else if alpha >= beta {
                                BoundType::Lower
                            } else {
                                BoundType::Exact
                            },
                            principal_variation: principal_variation.clone(),
                            lifetime: 2,
                        }
                    );
                    self.stats.last().unwrap().borrow_mut().tt_stores += 1;
                },
                _ => (),
            },
            None => (),
        }

        alpha
    }

    fn analyze(&mut self, state: &State) -> Vec<Ply> {
        let mut principal_variation = Vec::new();

        self.history.borrow_mut().clear();
        self.stats.clear();

        let start_move = time::precise_time_ns();

        let max_depth = if self.depth == 0 {
            15
        } else {
            self.depth
        };

        let precalculated = match self.transposition_table.borrow().get(&state.get_signature()) {
            Some(entry) => {
                if entry.bound_type == BoundType::Exact {
                    principal_variation.append(&mut entry.principal_variation.clone());
                    entry.depth
                } else {
                    0
                }
            },
            None => 0,
        };

        for depth in 1..precalculated + 1 {
            self.stats.push(RefCell::new(Statistics::new(depth)));
        }

        // Purge transposition table
        {
            let mut forget = Vec::with_capacity(400000);

            for (key, entry) in self.transposition_table.borrow_mut().iter_mut() {
                if entry.lifetime > 0 {
                    entry.lifetime -= 1;
                } else {
                    forget.push(key.clone());
                }
            }

            let mut transposition_table = self.transposition_table.borrow_mut();
            for key in forget {
                transposition_table.remove(&key);
            }
        }

        for depth in 1..max_depth + 1 - precalculated {
            self.stats.push(RefCell::new(Statistics::new(depth)));

            let start_search = time::precise_time_ns();

            let search_depth = depth + precalculated;
            let eval = self.minimax(state, &mut principal_variation, search_depth, search_depth, MIN_EVAL, MAX_EVAL);

            if eval.abs() > WIN_THRESHOLD {
                break;
            }

            let elapsed_search = (time::precise_time_ns() - start_search) as f32 / 1000000000.0;
            let elapsed_move = (time::precise_time_ns() - start_move) as f32 / 1000000000.0;

            // Use a simple branching factor of 12, for now
            if self.goal != 0 && elapsed_move + elapsed_search * 12.0 > self.goal as f32 {
                break;
            }
        }

        principal_variation
    }
}

pub struct MinimaxBot {
    ai: Arc<Mutex<Minimax>>,
}

impl MinimaxBot {
    pub fn new(depth: u8) -> MinimaxBot {
        MinimaxBot {
            ai: Arc::new(Mutex::new(Minimax::new(depth, 60))),
        }
    }
}

impl Ai for MinimaxBot {
    fn analyze(&mut self, state: &State) -> Vec<Ply> {
        self.ai.lock().unwrap().analyze(state)
    }

    fn get_stats(&self) -> Box<fmt::Display> {
        struct StatisticPrinter(Vec<Statistics>);

        impl fmt::Display for StatisticPrinter {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let result = write!(f, "Minimax Statistics:");
                for stats in self.0.iter() {
                    write!(f, "\n  Depth {}:\n", stats.depth).ok();
                    write!(f, "    {:21} {:14}\n", "Visited:", stats.visited).ok();
                    write!(f, "    {:21} {:14}\n", "Evaluated:", stats.evaluated).ok();
                    write!(f, "    {:21} {:4}/{:4}/{:4}", "TT Saves/Hits/Stores:", stats.tt_saves, stats.tt_hits, stats.tt_stores).ok();
                }
                result
            }
        }

        Box::new(StatisticPrinter(
            self.ai.lock().unwrap().stats.iter().map(|stats| stats.borrow().clone()).collect::<Vec<Statistics>>()
        ))
    }

    fn as_player(&self) -> &Player {
        self
    }
}

impl Player for MinimaxBot {
    fn initialize(&mut self, sender: Sender<Message>, receiver: Receiver<Message>, _: &Player) -> Result<(), String> {
        let ai = self.ai.clone();
        let cancel = ai.lock().unwrap().cancel.clone();
        let mut undos = 1;

        thread::spawn(move || {
            for message in receiver.iter() {
                match message {
                    Message::MoveRequest(state, _) => {
                        let sender = sender.clone();
                        let ai = ai.clone();
                        let cancel = cancel.clone();

                        {
                            let mut cancel = cancel.lock().unwrap();
                            if *cancel == true {
                                *cancel = false;
                            }
                        }

                        thread::spawn(move || {
                            let old_time = time::precise_time_ns();
                            let plies = ai.lock().unwrap().analyze(&state);
                            let elapsed_time = time::precise_time_ns() - old_time;

                            let mut cancel = cancel.lock().unwrap();
                            if *cancel == false {
                                println!("[MinimaxBot] Decision time (depth {}): {:.3} seconds", plies.len(), elapsed_time as f32 / 1000000000.0);

                                sender.send(Message::MoveResponse(plies[0].clone())).ok();
                            } else {
                                *cancel = false;
                            }
                        });
                    },
                    Message::UndoRequest => if undos > 0 {
                        undos -= 1;
                        *cancel.lock().unwrap() = true;
                        sender.send(Message::UndoRequest).ok();
                        sender.send(Message::Undo).ok();
                    },
                    _ => (),
                }
            }
        });

        Ok(())
    }

    fn get_name(&self) -> String {
        format!("Takkerus v{} (MinimaxBot - Depth: {})",
            option_env!("CARGO_PKG_VERSION").unwrap_or("Unknown"),
            self.ai.lock().unwrap().depth,
        )
    }

    fn as_any(&self) -> &Any {
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
enum BoundType {
    Lower,
    Exact,
    Upper,
}

struct TranspositionTableEntry {
    depth: u8,
    value: Eval,
    bound_type: BoundType,
    principal_variation: Vec<Ply>,
    lifetime: u8,
}

#[derive(Clone)]
pub struct Statistics {
    depth: u8,
    visited: u32,
    evaluated: u32,
    tt_saves: u32,
    tt_hits: u32,
    tt_stores: u32,
}

impl Statistics {
    pub fn new(depth: u8) -> Statistics {
        Statistics {
            depth: depth,
            visited: 0,
            evaluated: 0,
            tt_saves: 0,
            tt_hits: 0,
            tt_stores: 0,
        }
    }
}

struct PlyGenerator<'a> {
    ai: &'a Minimax,
    state: &'a State,
    principal_ply: Option<Ply>,
    plies: Vec<Ply>,
    operation: u8,
}

impl<'a> PlyGenerator<'a> {
    fn new(ai: &'a Minimax, state: &'a State, principal_ply: Option<Ply>) -> PlyGenerator<'a> {
        PlyGenerator {
            ai: ai,
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
                RNG.lock().unwrap().shuffle(self.plies.as_mut_slice());

                {
                    let history = self.ai.history.borrow();

                    if !history.is_empty() {
                        self.plies.sort_by(|a, b| {
                            history.get(&a.hash()).unwrap_or(&0).cmp(
                            history.get(&b.hash()).unwrap_or(&0))
                        });
                    }
                }
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

const END_GAME_FLATSTONE_THRESHOLD: [i32; 9] = [0, 0, 0, 5, 8, 10, 15, 20, 25];

struct Weights {
    flatstone: (Eval, Eval),
    standing_stone: Eval,
    capstone: Eval,

    hard_flat: (Eval, Eval, Eval),
    soft_flat: (Eval, Eval, Eval),

    threat: Eval,

    liberty: Eval,

    cover: (Eval, Eval, Eval),

    group: [Eval; 8],
}

const WEIGHT: Weights = Weights {
    flatstone:         (400, 800),
    standing_stone:     200,
    capstone:           300,

    hard_flat:         (125, 125, 150),
    soft_flat:         (-75, -50, -25),

    threat:             200,

    liberty:             20,

    cover:              (20, 15, -10),

    group: [0, 0, 100, 200, 400, 600, 0, 0],
};

pub trait Evaluatable {
    fn evaluate(&self) -> Eval;
    fn evaluate_plies(&self, plies: &[Ply]) -> Eval;
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

        let total_pieces = a.p1_pieces | a.p2_pieces;

        let p1_flatstones = a.p1_pieces & !a.standing_stones & !a.capstones;
        let p2_flatstones = a.p2_pieces & !a.standing_stones & !a.capstones;

        let p1_standing_stones = a.p1_pieces & a.standing_stones;
        let p2_standing_stones = a.p2_pieces & a.standing_stones;

        let p1_capstones = a.p1_pieces & a.capstones;
        let p2_capstones = a.p2_pieces & a.capstones;

        let (p1_flatstone_weight, p2_flatstone_weight) = {
            let flatstone_threshold = END_GAME_FLATSTONE_THRESHOLD[a.board_size];

            let p1_position = cmp::min(self.p1.flatstone_count as i32, flatstone_threshold);
            let p2_position = cmp::min(self.p2.flatstone_count as i32, flatstone_threshold);

            (
                WEIGHT.flatstone.0 * p1_position / flatstone_threshold +
                WEIGHT.flatstone.1 * (flatstone_threshold - p1_position) / flatstone_threshold,
                WEIGHT.flatstone.0 * p2_position / flatstone_threshold +
                WEIGHT.flatstone.1 * (flatstone_threshold - p2_position) / flatstone_threshold,
            )
        };

        // Top-level pieces
        p1_eval += a.p1_flatstone_count as i32 * p1_flatstone_weight;
        p2_eval += a.p2_flatstone_count as i32 * p2_flatstone_weight;

        p1_eval += p1_standing_stones.get_population() as i32 * WEIGHT.standing_stone;
        p2_eval += p2_standing_stones.get_population() as i32 * WEIGHT.standing_stone;

        p1_eval += p1_capstones.get_population() as i32 * WEIGHT.capstone;
        p2_eval += p2_capstones.get_population() as i32 * WEIGHT.capstone;

        // Stacked flatstones
        let mut p1_flatstone_hard_flats = -(a.p1_flatstone_count as i32); // Top-level flatstones don't count
        let mut p1_flatstone_soft_flats = 0;

        let mut p1_standing_stone_hard_flats = 0;
        let mut p1_standing_stone_soft_flats = 0;

        let mut p1_capstone_hard_flats = 0;
        let mut p1_capstone_soft_flats = 0;

        for level in a.p1_flatstones.iter() {
            if *level != 0 {
                p1_flatstone_hard_flats += (level & p1_flatstones).get_population() as i32;
                p1_flatstone_soft_flats += (level & p2_flatstones).get_population() as i32;

                p1_standing_stone_hard_flats += (level & p1_standing_stones).get_population() as i32;
                p1_standing_stone_soft_flats += (level & p2_standing_stones).get_population() as i32;

                p1_capstone_hard_flats += (level & p1_capstones).get_population() as i32;
                p1_capstone_soft_flats += (level & p2_capstones).get_population() as i32;
            }
        }

        let mut p2_flatstone_hard_flats = -(a.p2_flatstone_count as i32);
        let mut p2_flatstone_soft_flats = 0;

        let mut p2_standing_stone_hard_flats = 0;
        let mut p2_standing_stone_soft_flats = 0;

        let mut p2_capstone_hard_flats = 0;
        let mut p2_capstone_soft_flats = 0;

        for level in a.p2_flatstones.iter() {
            if *level != 0 {
                p2_flatstone_hard_flats += (level & p2_flatstones).get_population() as i32;
                p2_flatstone_soft_flats += (level & p1_flatstones).get_population() as i32;

                p2_standing_stone_hard_flats += (level & p2_standing_stones).get_population() as i32;
                p2_standing_stone_soft_flats += (level & p1_standing_stones).get_population() as i32;

                p2_capstone_hard_flats += (level & p2_capstones).get_population() as i32;
                p2_capstone_soft_flats += (level & p1_capstones).get_population() as i32;
            }
        }

        p1_eval += p1_flatstone_hard_flats * WEIGHT.hard_flat.0 + p2_flatstone_soft_flats * WEIGHT.soft_flat.0;
        p1_eval += p1_standing_stone_hard_flats * WEIGHT.hard_flat.1 + p2_standing_stone_soft_flats * WEIGHT.soft_flat.1;
        p1_eval += p1_capstone_hard_flats * WEIGHT.hard_flat.2 + p2_capstone_soft_flats * WEIGHT.soft_flat.2;

        p2_eval += p2_flatstone_hard_flats * WEIGHT.hard_flat.0 + p1_flatstone_soft_flats * WEIGHT.soft_flat.0;
        p2_eval += p2_standing_stone_hard_flats * WEIGHT.hard_flat.1 + p1_standing_stone_soft_flats * WEIGHT.soft_flat.1;
        p2_eval += p2_capstone_hard_flats * WEIGHT.hard_flat.2 + p1_capstone_soft_flats * WEIGHT.soft_flat.2;

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
        let evaluate_threats = |groups: &Vec<Bitmap>| {
            let mut expanded_groups = vec![0; groups.len()];
            let mut threats = 0;

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

            for i in 0..groups.len() {
                expanded_groups[i] = groups[i].grow(BOARD[a.board_size], a.board_size);
            }

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

        // Liberties
        let p1_liberties = (a.p1_pieces & !a.standing_stones).grow(BOARD[a.board_size] & !a.p2_pieces, a.board_size) & !a.p1_pieces;
        let p2_liberties = (a.p2_pieces & !a.standing_stones).grow(BOARD[a.board_size] & !a.p1_pieces, a.board_size) & !a.p2_pieces;

        p1_eval += p1_liberties.get_population() as i32 * WEIGHT.liberty;
        p2_eval += p2_liberties.get_population() as i32 * WEIGHT.liberty;

        // Cover
        let evaluate_cover = |pieces: Bitmap, own_flatstones: Bitmap, enemy_pieces: Bitmap| {
            let evaluate_half = |mut half_pieces: Bitmap| {
                fn pop_bit(map: Bitmap) -> (Bitmap, Bitmap) {
                    let remainder = map & (map - 1);
                    let bit = map & !remainder;
                    (bit, remainder)
                }

                if half_pieces == 0 {
                    return 0;
                }

                let expanded = half_pieces.grow(BOARD[a.board_size], a.board_size);

                let covered = {
                    let mut splat_board = 0;
                    loop {
                        let (bit, remainder) = pop_bit(half_pieces);

                        if bit != 0 {
                            let expanded_bit = bit.grow(BOARD[a.board_size], a.board_size);

                            splat_board ^= expanded_bit;
                        }

                        if remainder == 0 {
                            break;
                        }

                        half_pieces = remainder;
                    }

                    !splat_board & expanded
                };

                let mut eval = 0;
                eval += (covered & own_flatstones).get_population() as i32 * WEIGHT.cover.0;
                eval += (covered & !total_pieces).get_population() as i32 * WEIGHT.cover.1;
                eval += (covered & enemy_pieces).get_population() as i32 * WEIGHT.cover.2;
                eval
            };

            evaluate_half(pieces & 0x5555555555555555) + evaluate_half(pieces & 0xAAAAAAAAAAAAAAAA)
        };

        p1_eval += evaluate_cover(a.p1_pieces, p1_flatstones, a.p2_pieces);
        p2_eval += evaluate_cover(a.p2_pieces, p2_flatstones, a.p1_pieces);

        match next_color {
            Color::White => p1_eval - p2_eval,
            Color::Black => p2_eval - p1_eval,
        }
    }

    fn evaluate_plies(&self, plies: &[Ply]) -> Eval {
        let mut temp_state = self.clone();
        for ply in plies.iter() {
            match temp_state.execute_ply(ply) {
                Ok(next) => temp_state = next,
                Err(error) => panic!("Error calculating evaluation: {}, {}", error, ply.to_ptn()),
            }
        }
        temp_state.evaluate() * -((plies.len() as i32 % 2) * 2 - 1)
    }
}

#[cfg(test)]
mod tests {
    use std::f32;
    use time;

    use tak::*;
    use super::*;

    #[test]
    fn test_minimax() {
        let games = 50;
        let mut total_ply_count = 0;
        let mut total_min_time = f32::MAX;
        let mut total_max_time = 0.0;
        let mut total_time = 0.0;

        let mut p1_wins = 0;
        let mut p2_wins = 0;

        let mut draws = 0;
        let mut loops = 0;

        for _ in 1..(games + 1) {
            let mut state = State::new(5);

            let depth = 5;

            let mut p1 = Minimax::new(depth, 0);
            let mut p1_min_time = f32::MAX;
            let mut p1_max_time = 0.0;
            let mut p1_total_time = 0.0;

            let mut p2 = Minimax::new(depth, 0);
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

                let eval = state.evaluate_plies(&plies);

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
                    plies[0].to_ptn(),
                );

                if ply_count >= 150 {
                    loops += 1;
                    break 'game;
                }
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
                Win::Draw => draws += 1,
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
            println!("Black wins: {} / {}", p2_wins, games);
            println!("Draws: {} / {}", draws, games);
            println!("Loops: {} / {}\n", loops, games);
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
