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
mod arguments;
mod tak;

use std::env;
use std::fmt::Write;
use std::str::FromStr;

use ai::Ai;
use ai::minimax::Evaluatable;
use arguments::Type::*;
use tak::*;

enum Action {
    Play {
        state: State,
        p1: Box<Player>,
        p2: Box<Player>,
    },
    Analyze {
        state: State,
        ai: Box<Ai>,
    },
}

fn main() {
    let mut args = env::args().peekable();
    args.next();

    let next = match arguments::collect_next(&mut args, &[
        Flag("analyze"),
        Flag("play"),
        Flag("-h"),
        Flag("--help"),
    ]).ok() {
        Some(arguments) => arguments,
        None => panic!("Error collecting arguments"),
    };

    let action = if next.contains_key("-h") || next.contains_key("--help") {
        if next.contains_key("analyze") {
            println!("Usage:\n  takkerus analyze [-t string | -s int] [-a string [AI options]]\n");
            println!("Analyzes a board in TPS format or a blank board of the specified size, using the specified AI.");
            println!("    -t, --tps   string   Specifies a board in TPS format.");
            println!("    -s, --size  int      Specifies a blank board of Size. (default 5)");
            println!("    -a, --ai    string   The type of AI to use.  Options are:");
            println!("                           minimax (default)");
            println!("  Minimax options:");
            println!("    -d, --depth int      The depth of the search. (default 5)");
            println!("");
            return;
        } else if next.contains_key("play") {
            println!("Usage:\n  takkerus play [-t string | -s int] [-p1 string [Options]] [-p2 string [Options]]\n");
            println!("Starts a game of Tak between any combination of humans and AIs.");
            println!("    -t, --tps   string   Specifies a board in TPS format.");
            println!("    -s, --size  int      Specifies a blank board of Size. (default 5)");
            println!("    -p1         string   The type of player 1. Options are:");
            println!("                           human (default)");
            println!("                           minimax");
            println!("    -p2         string   The type of player 2. Options are:");
            println!("                           human");
            println!("                           minimax (default)");
            println!("  Human options:");
            println!("    -n, --name  string   The name of the player to record. (default Human)");
            println!("  Minimax options:");
            println!("    -d, --depth int      The depth of the search. (default 5)");
            println!("");
            return;
        } else {
            println!("Usage:\n  takkerus [Command [Command options]]\n");
            println!("A Tak program that can either analyze a given board, or play a game of Tak between two players, any combination of humans and AIs.");
            println!("  Commands:");
            println!("    analyze    Use an AI to analyze a board.");
            println!("    play       Start a game between any combination of humans and AIs. (default)");
            println!("\n  Use 'takkerus Command --help' for more info on Command.");
            return;
        }
    } else if next.contains_key("analyze") {
        let mut state = State::new(5);
        let mut ai = Box::new(ai::MinimaxBot::new(5));

        let next = match arguments::collect_next(&mut args, &[
            Option("-s", "--size", 1),
            Option("-t", "--tps", 1),
        ]) {
            Ok(arguments) => arguments,
            Err(error) => {
                println!("  Error: {}", error);
                return;
            },
        };

        match next.get("--size") {
            Some(strings) => {
                let size = match usize::from_str(&strings[0]) {
                    Ok(size) => if size >= 3 && size <= 8 {
                        size
                    } else {
                        println!("  Error: Invalid size.");
                        return;
                    },
                    _ => {
                        println!("  Error: Invalid size.");
                        return;
                    },
                };

                state = State::new(size);
            },
            None => match next.get("--tps") {
                Some(strings) => state = match State::from_tps(&strings[0]) {
                    Some(state) => state,
                    None => {
                        println!("  Error:  Invalid TPS.");
                        return;
                    },
                },
                None => (),
            },
        }

        let next = match arguments::collect_next(&mut args, &[Option("-a", "--ai", 1)]) {
            Ok(arguments) => arguments,
            Err(error) => {
                println!("  Error: {}", error);
                return;
            },
        };

        match next.get("--ai") {
            Some(strings) => if strings[0] == "minimax" {
                let mut depth = 5;

                let next = match arguments::collect_next(&mut args, &[Option("-d", "--depth", 1)]) {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        println!("  Error: {}", error);
                        return;
                    },
                };

                match next.get("--depth") {
                    Some(strings) => depth = match u8::from_str(&strings[0]) {
                        Ok(depth) => if depth > 0 && depth <= 10 {
                            depth
                        } else {
                            println!("  Error: Invalid minimax search depth.");
                            return;
                        },
                        _ => {
                            println!("  Error: Invalid minimax search depth.");
                            return;
                        },
                    },
                    None => (),
                };

                ai = Box::new(ai::MinimaxBot::new(depth));
            } else {
                println!("  Error: Invalid AI type.");
                return;
            },
            None => (),
        }

        Action::Analyze {
            state: state,
            ai: ai,
        }
    } else {
        let mut state = State::new(5);
        let mut p1: Box<Player> = Box::new(cli_player::CliPlayer::new("Human"));
        let mut p2: Box<Player> = Box::new(ai::MinimaxBot::new(5));

        let next = match arguments::collect_next(&mut args, &[
            Option("-s", "--size", 1),
            Option("-t", "--tps", 1),
        ]) {
            Ok(arguments) => arguments,
            Err(error) => {
                println!("  Error: {}", error);
                return;
            },
        };

        match next.get("--size") {
            Some(strings) => {
                let size = match usize::from_str(&strings[0]) {
                    Ok(size) => if size >= 3 && size <= 8 {
                        size
                    } else {
                        println!("  Error: Invalid size.");
                        return;
                    },
                    _ => {
                        println!("  Error: Invalid size.");
                        return;
                    },
                };

                state = State::new(size);
            },
            None => match next.get("--tps") {
                Some(strings) => state = match State::from_tps(&strings[0]) {
                    Some(state) => state,
                    None => {
                        println!("  Error:  Invalid TPS.");
                        return;
                    },
                },
                None => (),
            },
        }

        let next = match arguments::collect_next(&mut args, &[Option("-p1", "-p1", 1)]) {
            Ok(arguments) => arguments,
            Err(error) => {
                println!("  Error: {}", error);
                return;
            },
        };

        match next.get("-p1") {
            Some(strings) => if strings[0] == "human" {
                let mut name = String::from("Human");

                let next = match arguments::collect_next(&mut args, &[Option("-n", "--name", 1)]) {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        println!("  Error: {}", error);
                        return;
                    },
                };

                match next.get("--name") {
                    Some(strings) => name = strings[0].clone(),
                    None => (),
                }

                p1 = Box::new(cli_player::CliPlayer::new(&name));
            } else if strings[0] == "minimax" {
                let mut depth = 5;

                let next = match arguments::collect_next(&mut args, &[Option("-d", "--depth", 1)]) {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        println!("  Error: {}", error);
                        return;
                    },
                };

                match next.get("--depth") {
                    Some(strings) => depth = match u8::from_str(&strings[0]) {
                        Ok(depth) => if depth > 0 && depth <= 10 {
                            depth
                        } else {
                            println!("  Error: Invalid minimax search depth.");
                            return;
                        },
                        _ => {
                            println!("  Error: Invalid minimax search depth.");
                            return;
                        },
                    },
                    None => (),
                };

                p1 = Box::new(ai::MinimaxBot::new(depth));
            } else {
                println!("  Error: Invalid player type.");
                return;
            },
            None => (),
        }

        let next = match arguments::collect_next(&mut args, &[Option("-p2", "-p2", 1)]) {
            Ok(arguments) => arguments,
            Err(error) => {
                println!("  Error: {}", error);
                return;
            },
        };

        match next.get("-p2") {
            Some(strings) => if strings[0] == "human" {
                let mut name = String::from("Human");

                let next = match arguments::collect_next(&mut args, &[Option("-n", "--name", 1)]) {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        println!("  Error: {}", error);
                        return;
                    },
                };

                match next.get("--name") {
                    Some(strings) => name = strings[0].clone(),
                    None => (),
                }

                p2 = Box::new(cli_player::CliPlayer::new(&name));
            } else if strings[0] == "minimax" {
                let mut depth = 5;

                let next = match arguments::collect_next(&mut args, &[Option("-d", "--depth", 1)]) {
                    Ok(arguments) => arguments,
                    Err(error) => {
                        println!("  Error: {}", error);
                        return;
                    },
                };

                match next.get("--depth") {
                    Some(strings) => depth = match u8::from_str(&strings[0]) {
                        Ok(depth) => if depth > 0 && depth <= 10 {
                            depth
                        } else {
                            println!("  Error: Invalid minimax search depth.");
                            return;
                        },
                        _ => {
                            println!("  Error: Invalid minimax search depth.");
                            return;
                        },
                    },
                    None => (),
                };

                p2 = Box::new(ai::MinimaxBot::new(depth));
            } else {
                println!("  Error: Invalid player type.");
                return;
            },
            None => (),
        }

        Action::Play {
            state: state,
            p1: p1,
            p2: p2,
        }
    };

    match action {
        Action::Analyze {
            state,
            ai,
        } => analyze(state, ai),
        Action::Play {
            state,
            p1,
            p2,
        } => play(state, p1, p2),
    }
}

fn analyze(mut state: State, mut ai: Box<Ai>) {
    println!("{}", state);
    println!("{:?}\n", state.analysis);

    let old_time = time::precise_time_ns();

    let plies = ai.analyze(&state);

    let elapsed_time = (time::precise_time_ns() - old_time) as f32 / 1000000000.0;

    println!("Principal Variation:");
    for (i, ply) in plies.iter().enumerate() {
        println!("{}: {}", if (state.ply_count + i as u16) % 2 == 0 {
            "  White"
        } else {
            "  Black"
        }, ply.to_ptn());
    }

    println!("\n{}", ai.get_stats());

    let eval = {
        for ply in plies.iter() {
            match state.execute_ply(ply) {
                Ok(next) => state = next,
                Err(error) => panic!("Error calculating evaluation: {}", error),
            }
        }
        state.evaluate() * -((plies.len() as i32 % 2) * 2 - 1)
    };
    println!("\nEvaluation: {}", eval);

    println!("Time: {:.3}s", elapsed_time);
}

fn play(mut state: State, mut p1: Box<Player>, mut p2: Box<Player>) {
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
