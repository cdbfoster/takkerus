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
// Copyright 2016-2017 Chris Foster
//

#![feature(conservative_impl_trait)]

extern crate getopts;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate time;
extern crate zero_sum;

use std::env;
use std::str::FromStr;

use zero_sum::analysis::search::{PvSearch, Search};
use zero_sum::impls::tak::*;
use zero_sum::impls::tak::evaluator::StaticEvaluator;

use arguments::{Options, parse_player};
use game::{Game, logger};

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut main_options = Options::new();
    main_options
        .flag("h", "help")
        .flag("v", "version");

    let matches = main_options.parse(&args[1..]);

    if matches.opt_present("help") {
        println!("Usage:\n  takkerus [Command [Command options]]\n");
        println!("A playable tak board.  Supports play between any combination of humans and AIs, and can act as a PlayTak.com client.");
        println!("  Commands:");
        println!("    analyze    Use an AI to analyze a board.");
        println!("    play       Start a game between any combination of humans and AIs. (default)");
        println!("\n  Use 'takkerus Command --help' for more info on Command.");
        return;
    }

    if matches.opt_present("version") {
        println!("Takkerus{}", if let Some(version) = option_env!("CARGO_PKG_VERSION") {
            format!(" v{}", version)
        } else {
            String::from(" - version undefined")
        });
        return;
    }

    if !matches.free.is_empty() && matches.free[0] == "analyze" {
        let mut analyze_options = Options::new();
        analyze_options
            .flag("h", "help")
            .opt("f", "file")
            .opt("s", "size")
            .opt("a", "ai");

        let mut matches = analyze_options.parse(&matches.free[1..]);

        if matches.opt_present("help") {
            println!("Usage:\n  takkerus analyze [-f file | -s int] [-a string [AI options]]\n");
            println!("Analyzes a board in TPS format or a blank board of the specified size, using the specified AI.");
            println!("    -f, --file  FILE     Specifies a PTN file.");
            println!("    -s, --size  INT      Specifies a blank board of Size. (default 5)");
            println!("    -a, --ai    STRING   The type of AI to use.  Options are:");
            println!("                           pvsearch (default)");
            println!("  PVSearch options:");
            println!("    -d, --depth INT      The depth of the search.");
            println!("    -g, --goal  INT      The maximum search time in seconds. (default 60)");
            return;
        }

        if matches.opt_present("file") && matches.opt_present("size") {
            println!("  Error: Both file and size were specified.");
            return;
        }

        let state = if let Some(file_name) = matches.opt_str("file") {
            match logger::open_ptn_file(&file_name) {
                Ok((header, plies)) => match Game::into_state(header, plies) {
                    Ok(state) => state,
                    Err(error) => {
                        println!("  Error: {}", error);
                        return;
                    },
                },
                Err(error) => {
                    println!("  Error: {}", error);
                    return;
                },
            }
        } else if let Some(size) = matches.opt_str("size") {
            if let Ok(size) = usize::from_str(&size) {
                if size >= 3 && size <= 8 {
                    State::new(size)
                } else {
                    println!("  Error: Invalid size.");
                    return;
                }
            } else {
                println!("  Error: Invalid size.");
                return;
            }
        } else {
            State::new(5)
        };

        let mut search: Box<Search<State>> = if let Some(search) = matches.opt_str("ai") {
            if search == "pvsearch" {
                let mut pvsearch_options = Options::new();
                pvsearch_options
                    .opt("d", "depth")
                    .opt("g", "goal");

                matches = pvsearch_options.parse(&matches.free);

                if !matches.free.is_empty() {
                    println!("  Error: Unrecognized option: \"{}\".", matches.free[0]);
                    return;
                }

                if matches.opt_present("depth") && matches.opt_present("goal") {
                    println!("  Error: Both depth and goal were specified.");
                    return;
                }

                if let Some(depth) = matches.opt_str("depth") {
                    if let Ok(depth) = u8::from_str(&depth) {
                        Box::new(PvSearch::with_depth(StaticEvaluator, depth))
                    } else {
                        println!("  Error: Invalid depth.");
                        return;
                    }
                } else if let Some(goal) = matches.opt_str("goal") {
                    if let Ok(goal) = u16::from_str(&goal) {
                        Box::new(PvSearch::with_goal(StaticEvaluator, goal, 12.0))
                    } else {
                        println!("  Error: Invalid goal.");
                        return;
                    }
                } else {
                    Box::new(PvSearch::with_goal(StaticEvaluator, 60, 12.0))
                }
            } else {
                println!("  Error: Unrecognized AI type: {}.", search);
                return;
            }
        } else {
            Box::new(PvSearch::with_goal(StaticEvaluator, 60, 12.0))
        };

        if !matches.free.is_empty() {
            println!("  Error: Unrecognized option: \"{}\".", matches.free[0]);
            return;
        }

        println!("Analyzing state...");
        println!("Analysis:\n{}", search.search(&state, None));
    } else if !matches.free.is_empty() && matches.free[0] == "play" {
        let mut play_options = arguments::Options::new();
        play_options
            .flag("h", "help")
            .opt("s", "size");

        let mut matches = play_options.parse(&matches.free[1..]);

        if matches.opt_present("help") {
            println!("Usage:\n  takkerus play [-s int] [-p1 string [Options]] [-p2 string [Options]]\n");
            println!("Starts a game of Tak between any combination of humans and AIs.");
            println!("    -s, --size  INT      Specifies a blank board of Size. (default 5)");
            println!("        --p1    STRING   The type of player 1. Options are:");
            println!("                           human    (default)");
            println!("                           pvsearch");
            println!("                           playtak");
            println!("        --p2    STRING   The type of player 2. Options are:");
            println!("                           human");
            println!("                           pvsearch (default)");
            println!("                           playtak");
            println!("\n  Human options:");
            println!("    -n, --name  STRING   The name of the player to record. (default Human)");
            println!("\n  PVSearch options:");
            println!("    -d, --depth INT      The depth of the search.");
            println!("    -g, --goal  INT      The number of seconds per move to aim for. (default 60)");
            println!("\n  PlayTak options:");
            println!("    -h, --host  STRING   The host to connect to. (default \"playtak.com:10000\")");
            println!("    -u, --user  STRING   The username to log in with. (default \"\" (Guest login))");
            println!("    -p, --pass  STRING   The password to use. (default \"\" (Guest login))");
            println!("\n    --accept | --seek                         (default --seek)");
            println!("\n      Accept options:");
            println!("        -f, --from  STRING   The username to accept a game from.");
            println!("      Seek options:");
            println!("        -t, --time  INT      The number of seconds per player. (default 1200)");
            println!("        -i, --inc   INT      The post-turn increment in seconds. (default 30)");
            println!("        -c, --color STRING   The color to play. Options are:");
            println!("                               white");
            println!("                               black");
            println!("                               none (default)");
            return;
        }

        let mut game = Game::new();

        if let Some(size) = matches.opt_str("size") {
            if let Ok(size) = usize::from_str(&size) {
                if size >= 3 && size <= 8 {
                    game.header.size = size;
                } else {
                    println!("  Error: Invalid size.");
                    return;
                }
            } else {
                println!("  Error: Invalid size.");
                return;
            }
        }

        let mut p1_options = Options::new();
        p1_options
            .opt("", "p1");

        matches = p1_options.parse(&matches.free);

        let mut p1 = match parse_player(&game, "p1", &matches) {
            Ok((player, free)) => {
                matches.free = free;
                player
            },
            Err(error) => {
                println!("  Error: {}", error);
                return;
            },
        };

        let mut p2_options = Options::new();
        p2_options
            .opt("", "p2");

        matches = p2_options.parse(&matches.free);

        let p2 = match parse_player(&game, "p2", &matches) {
            Ok((player, free)) => {
                matches.free = free;
                player
            },
            Err(error) => {
                println!("  Error: {}", error);
                return;
            },
        };

        // Search for p1 again in case p2 was specified first
        if p1.is_none() {
            matches = p1_options.parse(&matches.free);

            p1 = match parse_player(&game, "p1", &matches) {
                Ok((player, _)) => player,
                Err(error) => {
                    println!("  Error: {}", error);
                    return;
                },
            };
        }

        if !matches.free.is_empty() {
            println!("  Error: Unrecognized option: \"{}\".", matches.free[0]);
            return;
        }

        game.add_player(p1.unwrap_or(Box::new(player::CliPlayer::new("Human")))).ok();
        game.add_player(p2.unwrap_or(Box::new(player::PvSearchPlayer::with_goal(60)))).ok();

        match game.play() {
            Err(error) => println!("  Error: {}", error),
            _ => (),
        }
    } else if matches.free.is_empty() {
        let mut game = Game::new();

        let p1 = Box::new(player::CliPlayer::new("Human"));
        let p2 = Box::new(player::PvSearchPlayer::with_goal(60));

        game.add_player(p1).ok();
        game.add_player(p2).ok();

        game.play().ok();
    } else {
        println!("  Error: Unrecognized option: \"{}\".", matches.free[0]);
    }
}

mod arguments;
mod game;
mod player;
