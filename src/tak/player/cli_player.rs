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
use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use tak::{Color, Message, Piece, Player, Ply, State};

pub struct CliPlayer {
    name: String,
}

impl CliPlayer {
    pub fn new(name: &str) -> CliPlayer {
        CliPlayer {
            name: String::from(name),
        }
    }
}

impl Player for CliPlayer {
    fn initialize(&mut self, sender: Sender<Message>, receiver: Receiver<Message>, opponent: &Player) -> Result<(), ()> {
        let share_stdin = if opponent.as_any().is::<CliPlayer>() {
            true
        } else {
            false
        };

        let (game_start_sender, game_start_receiver) = mpsc::channel();

        #[derive(Clone)]
        struct Data {
            state: Arc<Mutex<State>>,
            wait_move: Arc<Mutex<bool>>,
            wait_undo: Arc<Mutex<bool>>,
        }

        let data = Data {
            state: Arc::new(Mutex::new(State::new(5))),
            wait_move: Arc::new(Mutex::new(false)),
            wait_undo: Arc::new(Mutex::new(false)),
        };

        {
            let sender = sender.clone();
            let data = data.clone();

            thread::spawn(move || {
                let mut game_started = false;
                let mut child = None;

                for message in receiver.iter() {
                    if !waiting(&data) {
                        child = None;
                    }

                    match message {
                        Message::GameStart => {
                            game_start_sender.send(()).ok();
                            game_started = true;
                        },
                        Message::MoveRequest(ref new_state) => {
                            *data.state.lock().unwrap() = new_state.clone();
                            *data.wait_move.lock().unwrap() = true;
                        },
                        Message::UndoRequest => {
                            *data.wait_undo.lock().unwrap() = true;
                            *data.wait_move.lock().unwrap() = false;
                        },
                        Message::UndoResponse(undo) => if undo {
                            println!("  Your undo request was accepted.");
                        } else {
                            println!("  Your undo request was rejected.");
                        },
                        _ => (),
                    }

                    if game_started {
                        match message {
                            Message::MoveRequest(_) |
                            Message::UndoRequest => {
                                prompt(&data);
                            },
                            _ => (),
                        }

                        if share_stdin && child.is_none() && waiting(&data) {
                            let sender = sender.clone();
                            let data = data.clone();

                            child = Some(thread::spawn(move || {
                                loop {
                                    if waiting(&data) {
                                        process_command(&sender, &data);
                                    } else {
                                        break;
                                    }
                                }
                            }));
                        }
                    }
                }
            });
        }

        if !share_stdin {
            let sender = sender.clone();
            let data = data.clone();

            thread::spawn(move || {
                game_start_receiver.recv().ok();

                loop {
                    process_command(&sender, &data);
                }
            });
        }

        fn waiting(data: &Data) -> bool {
            *data.wait_move.lock().unwrap() ||
            *data.wait_undo.lock().unwrap()
        }

        fn process_command(sender: &Sender<Message>, data: &Data) {
            loop {
                let input = {
                    let mut input = String::new();
                    match io::stdin().read_line(&mut input) {
                        Ok(_) => input.trim().to_string(),
                        Err(error) => panic!("Error: {}", error),
                    }
                };

                {
                    let mut wait_undo = data.wait_undo.lock().unwrap();
                    if *wait_undo == true {
                        if input == "accept" {
                            *wait_undo = false;
                            sender.send(Message::UndoResponse(true)).ok();
                            break;
                        } else if input == "reject" {
                            *wait_undo = false;
                            sender.send(Message::UndoResponse(false)).ok();
                            break;
                        }
                    } else if input == "undo" {
                        *data.wait_move.lock().unwrap() = false;
                        sender.send(Message::UndoRequest).ok();
                        break;
                    }
                }

                if input == "help" {
                    println!("Commands:");
                    println!("  undo   - Requests an undo from your opponent of the most recent move.");
                    println!("  [move] - Enters your move in PTN format, e.g. a1, or 3d3<12.");
                    prompt(&data);
                    break;
                }

                {
                    let mut wait_move = data.wait_move.lock().unwrap();
                    if *wait_move == true {
                        match parse_ply(&input, &*data.state.lock().unwrap()) {
                            Some(ply) => {
                                *wait_move = false;
                                sender.send(Message::MoveResponse(ply)).ok();
                                break;
                            },
                            None => (),
                        }
                    }
                }

                prompt(&data);
            }
        }

        fn prompt(data: &Data) {
            if *data.wait_undo.lock().unwrap() == true {
                println!("Your opponent requested to undo the last move.");
                print!("  Enter response (\"accept\" or \"reject\"): ");
            } else {
                print!("Enter command: ");
            }

            io::stdout().flush().ok();
        }

        fn parse_ply(string: &str, state: &State) -> Option<Ply> {
            let board_size = state.board.len();

            let player_color = if state.ply_count % 2 == 0 {
                Color::White
            } else {
                Color::Black
            };

            let ply = match Ply::from_ptn(string, player_color) {
                Some(mut ply) => {
                    if state.ply_count < 2 {
                        ply = match ply {
                            Ply::Place { piece: Piece::Flatstone(color), x, y } => Ply::Place {
                                x: x,
                                y: y,
                                piece: Piece::Flatstone(color.flip()),
                            },
                            _ => {
                                println!("  Illegal opening move.");
                                return None;
                            },
                        };
                    }

                    match ply {
                        Ply::Place { x, y, .. } |
                        Ply::Slide { x, y, .. } => if x >= board_size || y >= board_size {
                            println!("  Out of bounds.");
                            return None;
                        }
                    }

                    ply
                },
                None => {
                    println!("  Invalid entry.");
                    return None;
                },
            };

            Some(ply)
        }

        Ok(())
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn as_any(&self) -> &Any {
        self
    }
}
