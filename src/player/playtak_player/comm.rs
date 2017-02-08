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

use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use zero_sum::analysis::search::{PvSearch, Search};
use zero_sum::impls::tak::*;
use zero_sum::State as StateTrait;

use game::Message;

use super::game_type::{GameType, ListedGame, Seek};
use super::message_queue::MessageQueue;
use super::parse;
use super::PlayTakPlayer;

pub fn write_stream(stream: &mut TcpStream, arguments: &[&str]) -> Result<(), io::Error> {
    try!(write!(*stream, "{}\n", arguments.join(" ")));
    try!(stream.flush());
    Ok(())
}

pub fn start_reader(stream: TcpStream, message_queue: &MessageQueue) {
    let mut message_queue = message_queue.clone();

    thread::spawn(move || {
        let reader = BufReader::new(stream);

        for line in reader.lines() {
            //if let Ok(ref message) = line {
            //    println!("Incoming: {}", message.trim());
            //}
            message_queue.push(line.unwrap().trim().to_string());
        }

        message_queue.disconnect();
    });
}

pub fn start_pinger(stream: &Arc<Mutex<TcpStream>>) {
    let stream = stream.clone();

    thread::spawn(move || {
        loop {
            thread::sleep(Duration::new(30, 0));

            let mut stream = stream.lock().unwrap();

            if let Err(_) = write_stream(&mut *stream, &["PING"]) {
                break;
            }
        }
    });
}

pub fn write_client_name(stream: &Arc<Mutex<TcpStream>>) {
    write_stream(&mut *stream.lock().unwrap(), &[
        "Client",
        &format!(
            "Takkerus{}",
            if let Some(version) = option_env!("CARGO_PKG_VERSION") {
                format!(" v{}", version)
            } else {
                String::new()
            }
        ),
    ]).ok();
}

pub fn login(stream: &Arc<Mutex<TcpStream>>, message_queue: &MessageQueue, player: &mut PlayTakPlayer) -> Result<(), String> {
    for message in message_queue.iter() {
        if message == "Login or Register" {
            break;
        }
    }

    {
        let login = {
            let login_name = if !player.username.is_empty() {
                &player.username
            } else {
                "Guest"
            };

            let mut login = vec!["Login", login_name];
            if !player.password.is_empty() {
                login.push(&player.password);
            }
            login
        };

        write_stream(&mut *stream.lock().unwrap(), &login).ok();
    }

    for message in message_queue.iter() {
        if message == "Login or Register" {
            return Err(String::from("Bad login."));
        }
        if message == "Authentication failure" {
            return Err(format!("Bad password: {}", player.password));
        }
        if message == "You're already logged in" { // XXX Does the server still send this?
            return Err(format!("User {} is already logged in.", player.username));
        }
        if message.starts_with("Welcome ") {
            if player.username.is_empty() {
                player.username = parse::welcome(&message).unwrap();
            }

            break;
        }
    }

    Ok(())
}

pub fn initialize_game(stream: &Arc<Mutex<TcpStream>>, message_queue: &MessageQueue, player: &mut PlayTakPlayer) -> Vec<ListedGame>{
    let mut game_list: Vec<ListedGame> = Vec::new();
    let message = message_queue.iter().peek().unwrap();

    // Are we already in a game?
    if message.starts_with("Game Start") {
        let (id, name, size, color) = parse::game_start(&message_queue.iter().next().unwrap()); // Consume the peeked message

        *player.resume_plies.lock().unwrap() = Some(parse::game(message_queue, &id));

        *player.id.lock().unwrap() = id;
        *player.name.lock().unwrap() = name;
        *player.size.lock().unwrap() = size;
        *player.color.lock().unwrap() = color;
    } else {
        if let GameType::Seek(ref seek) = player.game_type {
            post_seek(stream, seek);
        }

        for message in message_queue.iter() {
            if let GameType::Accept(ref from) = player.game_type {
                if message.starts_with("Seek new") {
                    let parts = message.split_whitespace().collect::<Vec<_>>();

                    if from == parts[3] {
                        write_stream(&mut *stream.lock().unwrap(), &[
                            "Accept",
                            parts[2],
                        ]).ok();
                    }
                }
            }

            if message.starts_with("Shout") {
                if let Some((invoker, command, value)) = parse::shout(&message, &player.username) {
                    if let GameType::Seek(ref mut seek) = player.game_type {
                        if command == "size" {
                            if let Ok(new_size) = usize::from_str(&value) {
                                if new_size != seek.size && new_size >= 4 && new_size <= 6 {
                                    seek.size = new_size;
                                    post_seek(stream, seek);
                                }
                            }
                        } else if command == "color" {
                            let color = if value == "white" {
                                Some(Color::White)
                            } else if value == "black" {
                                Some(Color::Black)
                            } else if value == "none" {
                                None
                            } else {
                                seek.color
                            };

                            if color != seek.color {
                                seek.color = color;
                                post_seek(stream, seek);
                            }
                        }
                    }

                    if command == "evaluate" {
                        if let Some(index) = game_list.iter().position(|game| game.p1 == invoker || game.p2 == invoker) {
                            let game = &game_list[index];

                            write_stream(&mut *stream.lock().unwrap(), &[
                                "Observe",
                                game.id.split_at(5).1,
                            ]).ok();

                            let plies = parse::game(message_queue, &game.id);

                            write_stream(&mut *stream.lock().unwrap(), &[
                                "Unobserve",
                                game.id.split_at(5).1,
                            ]).ok();

                            let state = State::from_plies(game.size, &plies).unwrap();
                            let stream = stream.clone();

                            thread::spawn(move || {
                                evaluate_state(&stream, &state, Some(invoker));
                            });
                        }
                    }
                }
            }

            if message.starts_with("GameList") {
                if let Some((game, command)) = parse::game_list(&message) {
                    if command == "Add" {
                        game_list.push(game);
                    } else if command == "Remove" {
                        if let Some(index) = game_list.iter().position(|x| *x == game) {
                            game_list.remove(index);
                        }
                    }
                }
            }

            if message.starts_with("Game Start") {
                let (id, name, size, color) = parse::game_start(&message);

                *player.id.lock().unwrap() = id;
                *player.name.lock().unwrap() = name;
                *player.size.lock().unwrap() = size;
                *player.color.lock().unwrap() = color;
                break;
            }
        }
    }

    game_list
}

pub fn start_game_handler(stream: &Arc<Mutex<TcpStream>>, player: &mut PlayTakPlayer) -> (Sender<Message>, Arc<Mutex<State>>, Arc<Mutex<bool>>, Arc<Mutex<bool>>) {
    let (sender, receiver) = mpsc::channel();
    let state = Arc::new(Mutex::new(State::new(5)));
    let undo_request = Arc::new(Mutex::new(false));
    let undo_wait = Arc::new(Mutex::new(false));

    {
        let stream = stream.clone();
        let state = state.clone();
        let undo_request = undo_request.clone();
        let undo_wait = undo_wait.clone();
        let id = player.id.clone();
        let color = player.color.clone();

        thread::spawn(move || {
            for message in receiver.iter() {
                match message {
                    Message::GameStart(own_color) => {
                        *color.lock().unwrap() = own_color;
                    },
                    Message::MoveResponse(ply) => {
                        *undo_request.lock().unwrap() = false;

                        write_stream(&mut *stream.lock().unwrap(), &[
                            &id.lock().unwrap(),
                            &ply_to_playtak(&ply),
                        ]).ok();
                    },
                    Message::MoveRequest(new_state) => {
                        *state.lock().unwrap() = new_state;
                    },
                    Message::UndoRequest => if !(*undo_request.lock().unwrap()) { // If we haven't already requested an undo,
                        *undo_wait.lock().unwrap() = true;
                        write_stream(&mut *stream.lock().unwrap(), &[
                            &id.lock().unwrap(),
                            "RequestUndo",
                        ]).ok();
                    },
                    Message::UndoAccept => {
                        let mut undo_request = undo_request.lock().unwrap();
                        if *undo_request {
                            *undo_request = false;
                            write_stream(&mut *stream.lock().unwrap(), &[
                                &id.lock().unwrap(),
                                "RequestUndo",
                            ]).ok();
                        }
                    }
                    Message::UndoRemove => {
                        let mut undo_wait = undo_wait.lock().unwrap();
                        if *undo_wait {
                            *undo_wait = false;
                            write_stream(&mut *stream.lock().unwrap(), &[
                                &id.lock().unwrap(),
                                "RemoveUndo",
                            ]).ok();
                        }
                    },
                    Message::GameOver => {
                        write_stream(&mut *stream.lock().unwrap(), &[
                            "quit",
                        ]).ok();
                    },
                    _ => (),
                }
            }
        });
    }

    (sender, state, undo_request, undo_wait)
}

pub fn start_playtak_handler(stream: &Arc<Mutex<TcpStream>>, message_queue: MessageQueue, to_game: Sender<(Color, Message)>, state: Arc<Mutex<State>>, undo_request: Arc<Mutex<bool>>, undo_wait: Arc<Mutex<bool>>, player: &mut PlayTakPlayer, mut game_list: Vec<ListedGame>) {
    let stream = stream.clone();
    let color = player.color.clone();
    let username = player.username.clone();

    thread::spawn(move || {
        for message in message_queue.iter() {
            if message.starts_with("Shout") {
                if let Some((invoker, command, _)) = parse::shout(&message, &username) {
                    if command == "evaluate" {
                        // If the invoker is in a game that's not our game
                        let (state, invoker) = if game_list.iter().position(|game|
                            (game.p1 == invoker || game.p2 == invoker) &&
                            (game.p1 != username && game.p2 != username)
                        ).is_some() {
                            let index = game_list.iter().position(|game| game.p1 == invoker || game.p2 == invoker).unwrap();
                            let game = &game_list[index];

                            write_stream(&mut *stream.lock().unwrap(), &[
                                "Observe",
                                game.id.split_at(5).1,
                            ]).ok();

                            let plies = parse::game(&message_queue, &game.id);

                            write_stream(&mut *stream.lock().unwrap(), &[
                                "Unobserve",
                                game.id.split_at(5).1,
                            ]).ok();

                            (State::from_plies(game.size, &plies).unwrap(), Some(invoker))
                        } else {
                            (state.lock().unwrap().clone(), None)
                        };

                        let stream = stream.clone();

                        thread::spawn(move || {
                            evaluate_state(&stream, &state, invoker);
                        });
                    }

                    // XXX About
                }
            }

            if message.starts_with("GameList") {
                if let Some((game, command)) = parse::game_list(&message) {
                    if command == "Add" {
                        game_list.push(game);
                    } else if command == "Remove" {
                        if let Some(index) = game_list.iter().position(|x| *x == game) {
                            game_list.remove(index);
                        }
                    }
                }
            }

            let parts = message.split_whitespace().collect::<Vec<_>>();

            if parts.len() <= 1 {
                continue;
            }

            if parts[1] == "P" || parts[1] == "M" {
                let string = parts[1..].join(" ");

                let next_color = {
                    let next_color = if state.lock().unwrap().ply_count % 2 == 0 {
                        Color::White
                    } else {
                        Color::Black
                    };

                    if state.lock().unwrap().ply_count < 2 {
                        next_color.flip()
                    } else {
                        next_color
                    }
                };

                if let Some(ply) = parse::ply(&string, next_color) {
                    let mut state = state.lock().unwrap();
                    if let Ok(next) = state.execute_ply(&ply) {
                        *state = next;
                    }

                    to_game.send((*color.lock().unwrap(), Message::MoveResponse(ply))).ok();
                }
            } else if parts[1] == "Over" {
                if parts[2] == "1-0" || parts[2] == "0-1" {
                    to_game.send((*color.lock().unwrap(), Message::Special(parts[2].to_string()))).ok();
                    break;
                }
            } else if parts[1] == "RequestUndo" {
                *undo_request.lock().unwrap() = true;
                to_game.send((*color.lock().unwrap(), Message::UndoRequest)).ok();
            } else if parts[1] == "RemoveUndo" {
                let mut undo_request = undo_request.lock().unwrap();
                if *undo_request {
                    to_game.send((*color.lock().unwrap(), Message::UndoRemove)).ok();
                    *undo_request = false;
                }
            } else if parts[1] == "Undo" {
                let mut undo_wait = undo_wait.lock().unwrap();
                if *undo_wait {
                    *undo_wait = false;
                    to_game.send((*color.lock().unwrap(), Message::UndoAccept)).ok();
                }
            } else if parts[1] == "Abandoned." {
                break;
            }
        }

        to_game.send((*color.lock().unwrap(), Message::Special(String::from("Disconnected")))).ok();
    });
}

pub fn post_seek(stream: &Arc<Mutex<TcpStream>>, seek: &Seek) { // XXX Does this need to be public?
    let string = format!("{} {} {}",
        seek.size,
        seek.time,
        seek.increment
    );

    let mut seek_data = vec![
        "Seek",
        &string
    ];

    if let Some(color) = seek.color {
        seek_data.push(match color {
            Color::White => "W",
            Color::Black => "B",
        });
    }

    write_stream(&mut *stream.lock().unwrap(), &seek_data).ok();
}

fn evaluate_state(stream: &Arc<Mutex<TcpStream>>, state: &State, invoker: Option<String>) {
    write_stream(&mut *stream.lock().unwrap(), &[
        &format!("Shout Evaluating{}...", if let Some(ref invoker) = invoker {
            format!("{}'s game", invoker)
        } else {
            String::new()
        }),
    ]).ok();

    let mut search = PvSearch::<Evaluation, State, Ply, Resolution>::with_goal(10, 12.0);

    let start_search = Instant::now();
    let analysis = search.search(&state, None);
    let elapsed_search = start_search.elapsed();
    let elapsed_search = elapsed_search.as_secs() as f32 + elapsed_search.subsec_nanos() as f32 / 1_000_000_000.0;

    write_stream(&mut *stream.lock().unwrap(), &[
        &format!("Shout{}", if let Some(ref invoker) = invoker {
            format!(" {}'s game:", invoker)
        } else {
            String::new()
        }),
        &format!("Evaluation for {} on turn {} (depth: {}, time: {:.2}s): {}",
            if state.ply_count % 2 == 0 {
                "white"
            } else {
                "black"
            },
            state.ply_count / 2 + 1,
            analysis.principal_variation.len(),
            elapsed_search,
            analysis.evaluation,
        ),
    ]).ok();
}

fn ply_to_playtak(ply: &Ply) -> String {
    fn format_square(x: usize, y: usize) -> String {
        format!("{}{}",
            (x as u8 + 65) as char,
            (y as u8 + 49) as char,
        )
    }

    match *ply {
        Ply::Place { x, y, ref piece } => format!("P {}{}",
            format_square(x, y),
            match *piece {
                Piece::Flatstone(_) => "",
                Piece::StandingStone(_) => " W",
                Piece::Capstone(_) => " C",
            },
        ),
        Ply::Slide { x, y, direction, ref drops } => format!("M {} {}{}",
            format_square(x, y),
            {
                let (dx, dy) = direction.to_offset();
                let (tx, ty) = (x as i8 + dx * drops.len() as i8, y as i8 + dy * drops.len() as i8);
                format_square(tx as usize, ty as usize)
            },
            drops.iter().map(|drop| format!(" {}", drop)).collect::<Vec<_>>().join(""),
        ),
    }
}
