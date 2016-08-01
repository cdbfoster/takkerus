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

extern crate fnv;
#[macro_use]
extern crate lazy_static;
extern crate rand;
extern crate regex;
extern crate time;

mod ai;
mod arguments;
mod logger;
mod tak;

use std::env;
use std::io::{self, Write};
use std::mem;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use ai::Ai;
use ai::minimax::Evaluatable;
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
    use arguments::Type::*;

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
            println!("Usage:\n  takkerus analyze [-f file | -s int] [-a string [AI options]]\n");
            println!("Analyzes a board in TPS format or a blank board of the specified size, using the specified AI.");
            println!("    -f, --file  file     Specifies a PTN file.");
            println!("    -s, --size  int      Specifies a blank board of Size. (default 5)");
            println!("    -a, --ai    string   The type of AI to use.  Options are:");
            println!("                           minimax (default)");
            println!("  Minimax options:");
            println!("    -d, --depth int      The depth of the search. (default 5)");
            println!("");
            return;
        } else if next.contains_key("play") {
            println!("Usage:\n  takkerus play [-s int] [-p1 string [Options]] [-p2 string [Options]]\n");
            println!("Starts a game of Tak between any combination of humans and AIs.");
            println!("    -s, --size  int      Specifies a blank board of Size. (default 5)");
            println!("    -p1         string   The type of player 1. Options are:");
            println!("                           human (default)");
            println!("                           minimax");
            println!("                           playtak");
            println!("    -p2         string   The type of player 2. Options are:");
            println!("                           human");
            println!("                           minimax (default)");
            println!("                           playtak");
            println!("\n  Human options:");
            println!("    -n, --name  string   The name of the player to record. (default Human)");
            println!("\n  Minimax options:");
            println!("    -d, --depth int      The depth of the search. (default 5)");
            println!("\n  Playtak options:");
            println!("    -h, --host  string   The host to connect to. (default \"playtak.com:10000)\"");
            println!("    -u, --user  string   The username to log in with. (default \"\" (Guest login))");
            println!("    -p, --pass  string   The password to use. (default \"\" (Guest login))");
            println!("\n    --accept | --seek                         (default --seek)");
            println!("\n      Accept options:");
            println!("        -f, --from  string   The username to accept a game from.");
            println!("      Seek options:");
            println!("        -t, --time  int      The number of seconds per player. (default 1200)");
            println!("        -i, --inc   int      The post-turn increment in seconds. (default 30)");
            println!("        -c, --color string   The color to play. Options are:");
            println!("                               white");
            println!("                               black");
            println!("                               none (default)");
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
            Option("-f", "--file", 1),
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
            None => match next.get("--file") {
                Some(strings) => state = match logger::open_ptn_file(&strings[0]) {
                    Ok(game) => game.to_state().unwrap(),
                    Err(logger::PtnFileError(error)) => {
                        println!("  Error: {}", error);
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
        let mut size = 5;
        let mut state = State::new(size);
        let mut p1: Box<Player> = Box::new(cli_player::CliPlayer::new("Human"));
        let mut p2: Box<Player> = Box::new(ai::MinimaxBot::new(5));

        let next = match arguments::collect_next(&mut args, &[
            Option("-s", "--size", 1),
        ]) {
            Ok(arguments) => arguments,
            Err(error) => {
                println!("  Error: {}", error);
                return;
            },
        };

        match next.get("--size") {
            Some(strings) => {
                size = match usize::from_str(&strings[0]) {
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
            None => (),
        }

        let mut parse_player = |player: &'static str,  default: Box<Player>| -> Result<Box<Player>, String> {
            let next = match arguments::collect_next(&mut args, &[Option(player, player, 1)]) {
                Ok(arguments) => arguments,
                Err(error) => return Err(format!("  Error: {}", error)),
            };

            match next.get(player) {
                Some(strings) => if strings[0] == "human" {
                    let mut name = String::from("Human");

                    let next = match arguments::collect_next(&mut args, &[Option("-n", "--name", 1)]) {
                        Ok(arguments) => arguments,
                        Err(error) => return Err(format!("  Error: {}", error)),
                    };

                    match next.get("--name") {
                        Some(strings) => name = strings[0].clone(),
                        None => (),
                    }

                    Ok(Box::new(cli_player::CliPlayer::new(&name)))
                } else if strings[0] == "minimax" {
                    let mut depth = 5;

                    let next = match arguments::collect_next(&mut args, &[Option("-d", "--depth", 1)]) {
                        Ok(arguments) => arguments,
                        Err(error) => return Err(format!("  Error: {}", error)),
                    };

                    match next.get("--depth") {
                        Some(strings) => depth = match u8::from_str(&strings[0]) {
                            Ok(depth) => if depth <= 15 {
                                depth
                            } else {
                                return Err(String::from("  Error: Invalid minimax search depth."));
                            },
                            _ => return Err(String::from("  Error: Invalid minimax search depth.")),
                        },
                        None => (),
                    }

                    Ok(Box::new(ai::MinimaxBot::new(depth)))
                } else if strings[0] == "playtak" {
                    let mut host = String::from("playtak.com:10000");
                    let mut username = String::new();
                    let mut password = String::new();
                    let game_type;

                    let next = match arguments::collect_next(&mut args, &[
                        Option("-h", "--host", 1),
                        Option("-u", "--user", 1),
                        Option("-p", "--pass", 1),
                        Flag("--accept"),
                        Flag("--seek"),
                    ]) {
                        Ok(arguments) => arguments,
                        Err(error) => return Err(format!("  Error: {}", error)),
                    };

                    match next.get("--host") {
                        Some(strings) => host = strings[0].clone(),
                        None => (),
                    }

                    match next.get("--user") {
                        Some(strings) => username = strings[0].clone(),
                        None => (),
                    }

                    match next.get("--pass") {
                        Some(strings) => password = strings[0].clone(),
                        None => (),
                    }

                    if next.contains_key("--accept") {
                        let next = match arguments::collect_next(&mut args, &[Option("-f", "--from", 1)]) {
                            Ok(arguments) => arguments,
                            Err(error) => return Err(format!("  Error: {}", error)),
                        };

                        match next.get("--from") {
                            Some(strings) => game_type = playtak_player::GameType::Accept(strings[0].clone()),
                            None => return Err(String::from("  Error: No --from user specified.")),
                        }
                    } else {
                        let mut time = 1200;
                        let mut increment = 30;
                        let mut color = None;

                        let next = match arguments::collect_next(&mut args, &[
                            Option("-t", "--time", 1),
                            Option("-i", "--inc", 1),
                            Option("-c", "--color", 1),
                        ]) {
                            Ok(arguments) => arguments,
                            Err(error) => return Err(format!("  Error: {}", error)),
                        };

                        match next.get("--time") {
                            Some(strings) => time = match u32::from_str(&strings[0]) {
                                Ok(time) => if time > 0 {
                                    time
                                } else {
                                    return Err(String::from("  Error: Invalid player timer value."));
                                },
                                _ => return Err(String::from("  Error: Invalid player timer value.")),
                            },
                            None => (),
                        }

                        match next.get("--inc") {
                            Some(strings) => increment = match u32::from_str(&strings[0]) {
                                Ok(increment) => increment,
                                _ => return Err(String::from("  Error: Invalid player timer increment.")),
                            },
                            None => (),
                        }

                        match next.get("--color") {
                            Some(strings) => color = if strings[0] == "white" {
                                Some(Color::White)
                            } else if strings[0] == "black" {
                                Some(Color::Black)
                            } else if strings[0] == "none" {
                                None
                            } else {
                                return Err(String::from("  Error: Invalid player color."));
                            },
                            None => (),
                        }

                        game_type = playtak_player::GameType::Seek {
                            size: size,
                            time: time,
                            increment: increment,
                            color: color,
                        };
                    }

                    Ok(Box::new(playtak_player::PlaytakPlayer::new(
                        &host,
                        &username,
                        &password,
                        game_type
                    )))
                } else {
                    Err(String::from("  Error: Invalid player type."))
                },
                None => Ok(default),
            }
        };

        match parse_player("-p1", p1) {
            Ok(player) => p1 = player,
            Err(error) => {
                println!("{}", error);
                return;
            },
        }

        match parse_player("-p2", p2) {
            Ok(player) => p2 = player,
            Err(error) => {
                println!("{}", error);
                return;
            },
        }

        if p1.as_any().is::<playtak_player::PlaytakPlayer>() &&
           p2.as_any().is::<playtak_player::PlaytakPlayer>() {
            println!("  Error: Only one player may be of type \"playtak\".");
            return;
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
    let p1_playtak = p1.as_any().is::<playtak_player::PlaytakPlayer>();
    let p2_playtak = p2.as_any().is::<playtak_player::PlaytakPlayer>();
    let using_playtak = p1_playtak || p2_playtak;

    let (p1_game_sender, mut p1_game_receiver) = mpsc::channel();
    let (mut game_p1_sender, game_p1_receiver) = mpsc::channel();

    let (p2_game_sender, mut p2_game_receiver) = mpsc::channel();
    let (mut game_p2_sender, game_p2_receiver) = mpsc::channel();

    match p1.initialize(p1_game_sender, game_p1_receiver, &*p2) {
        Err(error) => {
            println!("  Error: Failed to initialize player 1: {}", error);
            return;
        },
        _ => (),
    }

    match p2.initialize(p2_game_sender, game_p2_receiver, &*p1) {
        Err(error) => {
            println!("  Error: Failed to initialize player 2: {}", error);
            return;
        },
        _ => (),
    }

    if using_playtak {
        let color = if p1_playtak {
            match p1.as_any().downcast_ref::<playtak_player::PlaytakPlayer>() {
                Some(player) => player.game_info.color.unwrap(),
                None => panic!("Player 1 isn't Playtak!"),
            }
        } else {
            match p2.as_any().downcast_ref::<playtak_player::PlaytakPlayer>() {
                Some(player) => player.game_info.color.unwrap(),
                None => panic!("Player 2 isn't Playtak!"),
            }
        };

        if (color == Color::White && p1_playtak) ||
           (color == Color::Black && p2_playtak) {
            mem::swap(&mut p1, &mut p2);
            mem::swap(&mut game_p1_sender, &mut game_p2_sender);
            mem::swap(&mut p1_game_receiver, &mut p2_game_receiver);
        }
    }

    let game = if !using_playtak {
        match logger::check_tmp_file() {
            logger::GameState::New(mut game) => {
                logger::populate_game(&mut game, &*p1, &*p2);
                game
            },
            logger::GameState::Resume(mut game) => {
                println!("There is a game in progress.\n");
                if !game.header.p1.is_empty() && !game.header.p2.is_empty() {
                    print!("  {} vs {}", game.header.p1, game.header.p2);
                }
                if !game.header.date.is_empty() {
                    print!("  {}", game.header.date);
                }
                println!("  {}x{}, turn {}\n", game.header.size, game.header.size, game.to_state().unwrap().ply_count / 2 + 1);

                println!("Resume game? (y/n)");
                loop {
                    print!("Option: ");
                    io::stdout().flush().ok();

                    let mut input = String::new();
                    match io::stdin().read_line(&mut input) {
                        Ok(_) => {
                            let response = input.trim().to_lowercase();
                            if response == "y" {
                                state = game.to_state().unwrap();
                                break;
                            } else if response == "n" {
                                game = logger::Game::new();
                                logger::populate_game(&mut game, &*p1, &*p2);
                                break;
                            }
                        },
                        Err(e) => panic!("Error: {}", e),
                    }
                }

                game
            },
        }
    } else {
        let mut game = logger::Game::new();
        logger::populate_game(&mut game, &*p1, &*p2);
        game.header.site = String::from("playtak.com");
        game
    };

    let state = Arc::new(Mutex::new(state));
    let game = Arc::new(Mutex::new(game));

    let (game_sender, game_receiver) = mpsc::channel();

    fn handle_player(state: Arc<Mutex<State>>, game: Arc<Mutex<logger::Game>>, own_sender: Sender<Message>, own_receiver: Receiver<Message>, opponent_sender: Sender<Message>, game_sender: Sender<Message>) {
        thread::spawn(move || {
            for message in own_receiver.iter() {
                match message {
                    Message::MoveResponse(ref ply) => {
                        let mut state = state.lock().unwrap();
                        match state.execute_ply(ply) {
                            Ok(next) => {
                                *state = next;

                                game.lock().unwrap().plies.push(ply.clone());

                                game_sender.send(message.clone()).ok();
                            },
                            Err(error) => {
                                println!("  {}", error);
                                own_sender.send(Message::MoveRequest(state.clone(), None)).ok();
                            },
                        }
                    },
                    Message::UndoRequest => {
                        opponent_sender.send(Message::UndoRequest).ok();
                    },
                    Message::RemoveUndoRequest => {
                        opponent_sender.send(Message::RemoveUndoRequest).ok();
                    },
                    Message::Undo => {
                        let mut game = game.lock().unwrap();

                        game.plies.pop();
                        *state.lock().unwrap() = game.to_state().unwrap();

                        game_sender.send(message).ok();
                    },
                    Message::Quit(_) => {
                        opponent_sender.send(message.clone()).ok();
                        game_sender.send(message).ok();
                    },
                    Message::EarlyEnd(_) => {
                        opponent_sender.send(message.clone()).ok();
                        game_sender.send(message).ok();
                    },
                    _ => (),
                }
            }
        });
    }

    handle_player(state.clone(), game.clone(), game_p1_sender.clone(), p1_game_receiver, game_p2_sender.clone(), game_sender.clone());
    handle_player(state.clone(), game.clone(), game_p2_sender.clone(), p2_game_receiver, game_p1_sender.clone(), game_sender.clone());

    game_p1_sender.send(Message::GameStart(Color::White)).ok();
    game_p2_sender.send(Message::GameStart(Color::Black)).ok();

    game_sender.send(Message::GameStart(Color::White)).ok();
    let mut first_ply = true;

    'main: for message in game_receiver.iter() {
        let state = state.lock().unwrap();
        let mut game = game.lock().unwrap();

        println!("\n--------------------------------------------------");
        println!("{}", *state);

        let ptn = if state.ply_count % 2 == 0 {
            format!("{:<5} {}",
                if game.plies.len() >= 2 {
                    game.plies[game.plies.len() - 2].to_ptn()
                } else {
                    String::from("--")
                },
                match game.plies.last() {
                    Some(ply) => ply.to_ptn(),
                    None => String::new(),
                },
            )
        } else if game.plies.len() >= 1 {
            format!("{}", game.plies.last().unwrap().to_ptn())
        } else {
            String::from("--")
        };

        match state.check_win() {
            Win::None => (),
            _ => {
                println!("Final state:     {}\n", ptn);

                if let Message::MoveResponse(ply) = message {
                    if state.ply_count % 2 == 0 {
                        game_p1_sender.send(Message::FinalMove(state.clone(), ply)).ok();
                    } else {
                        game_p2_sender.send(Message::FinalMove(state.clone(), ply)).ok();
                    }
                }

                break 'main;
            },
        }

        if state.ply_count > 0 {
            println!("Previous {}:   {}\n", if state.ply_count % 2 == 0 {
                "turn"
            } else {
                "move"
            }, ptn);
        } else {
            println!("\n");
        }

        if let Message::Quit(color) = message.clone() {
            if color == Color::Black {
                println!("Player 1 wins. (1-0)");
                game.header.result = String::from("1-0");
            } else {
                println!("Player 2 wins. (0-1)");
                game.header.result = String::from("0-1");
            }
            break 'main;
        }

        if let Message::EarlyEnd(string) = message.clone() {
            if string == "1-0" {
                println!("Player 1 wins. (1-0)");
            } else if string == "0-1" {
                println!("Player 2 wins. (0-1)");
            }
            game.header.result = string;
            break 'main;
        }

        println!("Turn {} ({})", state.ply_count / 2 + 1, if state.ply_count % 2 == 0 {
            "White"
        } else {
            "Black"
        });

        let ply = if let Message::MoveResponse(ply) = message {
            Some(ply)
        } else {
            None
        };

        if state.ply_count % 2 == 0 {
            game_p1_sender.send(Message::MoveRequest(state.clone(), ply)).ok();
        } else {
            game_p2_sender.send(Message::MoveRequest(state.clone(), ply)).ok();
        }

        if first_ply {
            first_ply = false;
        } else {
            logger::write_tmp_file(&*game);
        }
    }

    let state = state.lock().unwrap();
    let mut game = game.lock().unwrap();

    match state.check_win() {
        Win::Road(color) => match color {
            Color::White => { println!("Player 1 wins! (R-0)"); game.header.result = String::from("R-0"); },
            Color::Black => { println!("Player 2 wins! (0-R)"); game.header.result = String::from("0-R"); },
        },
        Win::Flat(color) => match color {
            Color::White => { println!("Player 1 wins! (F-0)"); game.header.result = String::from("F-0"); },
            Color::Black => { println!("Player 2 wins! (0-F)"); game.header.result = String::from("0-F"); },
        },
        Win::Draw => { println!("Draw! (1/2-1/2)"); game.header.result = String::from("1/2-1/2"); },
        _ => (),
    }

    logger::write_tmp_file(&*game);
    logger::finalize_tmp_file();

    thread::sleep(Duration::new(1, 0)); // Give things a bit of time to clear up.
}
