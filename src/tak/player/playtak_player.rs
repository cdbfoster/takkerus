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

#![allow(unused_imports)]
use std::any::Any;
use std::cmp;
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::str::FromStr;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use regex::Regex;

use tak::{Color, Direction, Message, Piece, Player, Ply, State};

lazy_static! {
    static ref SHOUT_COMMAND: Regex = Regex::new(
        "^Shout <[^>]+> ([^ :]+):?\\s*([^ :]+):?\\s*(.*)$"
    ).unwrap();
}

#[derive(Clone)]
pub enum GameType {
    Accept(String),
    Seek {
        size: usize,
        time: u32,
        increment: u32,
        color: Option<Color>,
    },
}

#[derive(Clone)]
pub struct GameInfo {
    pub id: String,
    pub name: String,
    pub size: usize,
    pub color: Option<Color>,
}

pub struct PlaytakPlayer {
    host: String,
    username: String,
    password: String,
    game: GameType,
    pub game_info: GameInfo,
}

impl PlaytakPlayer {
    pub fn new(host: &str, username: &str, password: &str, game: GameType) -> PlaytakPlayer {
        PlaytakPlayer {
            host: host.to_string(),
            username: username.to_string(),
            password: password.to_string(),
            game: game,
            game_info: GameInfo {
                id: String::new(),
                name: String::from("Playtak.com Player"),
                size: 5,
                color: None,
            },
        }
    }
}

impl Player for PlaytakPlayer {
    fn initialize(&mut self, sender: Sender<Message>, receiver: Receiver<Message>, _: &Player) -> Result<(), String> {
        let (initialize_sender, initialize_receiver) = mpsc::channel();

        let game_info = Arc::new(Mutex::new(GameInfo {
            id: String::new(),
            name: String::from("Playtak.com Player"),
            size: 5,
            color: None,
        }));

        let host = self.host.clone();
        let username = self.username.clone();
        let password = self.password.clone();
        let mut game = self.game.clone();
        let game_info_clone = game_info.clone();

        thread::spawn(move || {
            let stream = match TcpStream::connect(host.as_str()) {
                Ok(stream) => stream,
                Err(_) => {
                    initialize_sender.send(Err(String::from("Could not connect to host."))).ok();
                    return;
                },
            };

            println!("Connected.");

            let (connection_sender, connection_receiver) = mpsc::channel();

            // Reader
            {
                let stream = stream.try_clone().unwrap();

                thread::spawn(move || {
                    let reader = BufReader::new(stream);

                    for line in reader.lines() {
                        connection_sender.send(line.unwrap().trim().to_string()).ok();
                    }
                })
            };

            let stream = Arc::new(Mutex::new(stream));

            fn write_stream(stream: &mut TcpStream, arguments: &[&str]) -> Result<(), io::Error> {
                let result = write!(*stream, "{}\n", arguments.join(" "));
                stream.flush().ok();

                result
            };

            // Ping the server every 30 seconds
            {
                let stream = stream.clone();

                thread::spawn(move || {
                    loop {
                        thread::sleep(Duration::new(30, 0));

                        let mut stream = stream.lock().unwrap();

                        match write_stream(&mut *stream, &["PING"]) {
                            Err(_) => break,
                            _ => (),
                        }
                    }
                });
            }

            // Write client name
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

            // Login
            {
                for message in connection_receiver.iter() {
                    if message == "Login or Register" {
                        break;
                    }
                }

                let login_name = if !username.is_empty() {
                    &username
                } else {
                    "Guest"
                };

                let mut login = vec!["Login", login_name];
                if !password.is_empty() {
                    login.push(&password);
                }

                write_stream(&mut *stream.lock().unwrap(), &login).ok();

                for message in connection_receiver.iter() {
                    if message == "Login or Register" {
                        initialize_sender.send(Err(String::from("Bad login."))).ok();
                        return;
                    }
                    if message == "Authentication failure" {
                        initialize_sender.send(Err(format!("Bad password: {}", password))).ok();
                        return;
                    }
                    if message == "You're already logged in" { // XXX Does the server still send this?
                        initialize_sender.send(Err(format!("User {} is already logged in.", username))).ok();
                        return;
                    }
                    if message.starts_with("Welcome ") {
                        break;
                    }
                }
            }

            // Find/start a game
            {
                fn seek_game(stream: &Arc<Mutex<TcpStream>>, size: usize, time: u32, increment: u32, color: Option<Color>) {
                    let string = format!("{} {} {}",
                        size,
                        time,
                        increment
                    );
                    let mut seek = vec!["Seek", &string];

                    if let Some(color) = color {
                        seek.push(match color {
                            Color::White => "W",
                            Color::Black => "B",
                        });
                    }

                    write_stream(&mut *stream.lock().unwrap(), &seek).ok();
                }

                if let GameType::Seek { size, time, increment, color } = game.clone() {
                    seek_game(&stream, size, time, increment, color);
                }

                for message in connection_receiver.iter() {
                    if let GameType::Accept(from) = game.clone() {
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
                        match SHOUT_COMMAND.captures(&message) {
                            Some(captures) => {
                                let target = captures[1].to_lowercase();

                                if target != username.to_lowercase() &&
                                   format!("{}bot", target) != username.to_lowercase() {
                                    continue;
                                }

                                let command = captures[2].to_lowercase();
                                let value = captures[3].to_lowercase();

                                if let GameType::Seek { size, time, increment, color } = game.clone() {
                                    if command == "size" {
                                        if let Ok(new_size) = usize::from_str(&value) {
                                            if new_size != size && new_size >= 4 && new_size <= 6 {
                                                game = GameType::Seek {
                                                    size: new_size,
                                                    time: time,
                                                    increment: increment,
                                                    color: color,
                                                };
                                                seek_game(&stream, new_size, time, increment, color);
                                            }
                                        }
                                    } else if command == "color" {
                                        let mut new_color = color;

                                        if value == "white" {
                                            new_color = Some(Color::White);
                                        } else if value == "black" {
                                            new_color = Some(Color::Black);
                                        } else if value == "none" {
                                            new_color = None;
                                        }

                                        if new_color != color {
                                            game = GameType::Seek {
                                                size: size,
                                                time: time,
                                                increment: increment,
                                                color: new_color,
                                            };
                                            seek_game(&stream, size, time, increment, new_color);
                                        }
                                    }
                                }
                            },
                            None => (),
                        }
                    }

                    if message.starts_with("Game Start") {
                        let parts = message.split_whitespace().collect::<Vec<_>>();

                        *game_info_clone.lock().unwrap() = GameInfo {
                            id: format!("Game#{}", parts[2]),
                            name: if parts[7] == "white" {
                                parts[6].to_string()
                            } else {
                                parts[4].to_string()
                            },
                            size: usize::from_str(parts[3]).unwrap(),
                            color: if parts[7] == "white" {
                                Some(Color::White)
                            } else {
                                Some(Color::Black)
                            },
                        };

                        initialize_sender.send(Ok(())).ok();
                        break;
                    }
                }
            }

            let color = Arc::new(Mutex::new(Color::White));
            let state = Arc::new(Mutex::new(State::new(game_info_clone.lock().unwrap().size)));
            let request_undo = Arc::new(Mutex::new(false));

            // Game listener
            {
                let color = color.clone();
                let stream = stream.clone();
                let game_info = game_info_clone.clone();
                let state = state.clone();
                let request_undo = request_undo.clone();

                thread::spawn(move || {
                    for message in receiver.iter() {
                        match message {
                            Message::GameStart(own_color) => {
                                *color.lock().unwrap() = own_color;
                            },
                            Message::MoveRequest(new_state, Some(ply)) |
                            Message::FinalMove(new_state, ply) => {
                                *state.lock().unwrap() = new_state;
                                *request_undo.lock().unwrap() = false;

                                write_stream(&mut *stream.lock().unwrap(), &[
                                    &game_info.lock().unwrap().id,
                                    &ply_to_playtak(&ply),
                                ]).ok();
                            },
                            Message::UndoRequest => {
                                write_stream(&mut *stream.lock().unwrap(), &[
                                    &game_info.lock().unwrap().id,
                                    "RequestUndo",
                                ]).ok();
                            },
                            Message::RemoveUndoRequest => {
                                write_stream(&mut *stream.lock().unwrap(), &[
                                    &game_info.lock().unwrap().id,
                                    "RemoveUndo",
                                ]).ok();
                            },
                            Message::Quit(_) => {
                                write_stream(&mut *stream.lock().unwrap(), &[
                                    "quit",
                                ]).ok();
                            },
                            _ => (),
                        }
                    }
                });
            }

            for message in connection_receiver.iter() {
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

                    if let Some(ply) = playtak_to_ply(&string, next_color) {
                        sender.send(Message::MoveResponse(ply)).ok();
                    }
                } else if parts[1] == "Over" {
                    if parts[2] == "1-0" || parts[2] == "0-1" {
                        sender.send(Message::EarlyEnd(parts[2].to_string())).ok();
                    }
                } else if parts[1] == "RequestUndo" {
                    *request_undo.lock().unwrap() = true;
                    sender.send(Message::UndoRequest).ok();
                } else if parts[1] == "RemoveUndo" {
                    let mut request_undo = request_undo.lock().unwrap();
                    if *request_undo == true {
                        sender.send(Message::RemoveUndoRequest).ok();
                        *request_undo = false;
                    }
                } else if parts[1] == "Undo" {
                    if *request_undo.lock().unwrap() == false {
                        sender.send(Message::UndoRequest).ok();
                        sender.send(Message::Undo).ok();
                    }
                } else if parts[1] == "Abandoned." {
                    sender.send(Message::Quit(*color.lock().unwrap())).ok();
                }
            }

            sender.send(Message::Quit(*color.lock().unwrap())).ok(); // Disconnected
        });

        match initialize_receiver.recv().unwrap() {
            Ok(_) => {
                self.game_info = game_info.lock().unwrap().clone();
                Ok(())
            },
            Err(error) => Err(error),
        }
    }

    fn get_name(&self) -> String {
        self.game_info.name.clone()
    }

    fn as_any(&self) -> &Any {
        self
    }
}

fn ply_to_playtak(ply: &Ply) -> String {
    fn format_square(x: usize, y: usize) -> String {
        format!("{}{}",
            (x as u8 + 65) as char,
            (y as u8 + 49) as char,
        )
    }

    match ply {
        &Ply::Place { x, y, ref piece } => format!("P {}{}",
            format_square(x, y),
            match piece {
                &Piece::Flatstone(_) => "",
                &Piece::StandingStone(_) => " W",
                &Piece::Capstone(_) => " C",
            },
        ),
        &Ply::Slide { x, y, direction, ref drops } => format!("M {} {}{}",
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

fn playtak_to_ply(string: &str, color: Color) -> Option<Ply> {
    fn parse_square(square: &str) -> Option<(usize, usize)> {
        let mut chars = square.chars();

        let x = if let Some(x) = chars.next() {
            (x as u8 - 65) as usize
        } else {
            return None;
        };

        let y = if let Some(y) = chars.next() {
            (y as u8 - 49) as usize
        } else {
            return None;
        };

        Some((x, y))
    }

    let parts = string.split_whitespace().collect::<Vec<_>>();

    if parts[0] == "P" {
        if parts.len() < 2 {
            return None;
        }

        let (x, y) = if let Some(coordinates) = parse_square(parts[1]) {
            coordinates
        } else {
            return None;
        };

        let piece = if parts.len() >= 3 {
            if parts[2] == "W" {
                Piece::StandingStone(color)
            } else if parts[2] == "C" {
                Piece::Capstone(color)
            } else {
                return None;
            }
        } else {
            Piece::Flatstone(color)
        };

        Some(Ply::Place {
            x: x,
            y: y,
            piece: piece
        })
    } else if parts[0] == "M" {
        if parts.len() < 4 {
            return None;
        }

        let (x, y) = if let Some(coordinates) = parse_square(parts[1]) {
            coordinates
        } else {
            return None;
        };

        let (tx, ty) = if let Some(coordinates) = parse_square(parts[2]) {
            coordinates
        } else {
            return None;
        };

        let direction = {
            let (dx, dy) = (
                tx as i8 - x as i8,
                ty as i8 - y as i8,
            );

            if dx < 0 && dy == 0 {
                Direction::West
            } else if dx > 0 && dy == 0 {
                Direction::East
            } else if dy < 0 && dx == 0 {
                Direction::South
            } else if dy > 0 && dx == 0 {
                Direction::North
            } else {
                return None;
            }
        };

        let drops = parts[3..].iter().map(|drop| u8::from_str(drop).unwrap()).collect::<Vec<_>>();

        Some(Ply::Slide {
            x: x,
            y: y,
            direction: direction,
            drops: drops,
        })
    } else {
        None
    }
}