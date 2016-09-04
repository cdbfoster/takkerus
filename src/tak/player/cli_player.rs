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
    fn initialize(&mut self, sender: Sender<Message>, receiver: Receiver<Message>, opponent: &Player) -> Result<(), String> {
        let share_stdin = opponent.as_any().is::<CliPlayer>();

        let (game_start_sender, game_start_receiver) = mpsc::channel();

        #[derive(Clone)]
        struct Data {
            color: Arc<Mutex<Color>>,
            state: Arc<Mutex<State>>,
            wait_move: Arc<Mutex<bool>>,
            wait_undo: Arc<Mutex<bool>>,
            request_undo: Arc<Mutex<bool>>,
        }

        let data = Data {
            color: Arc::new(Mutex::new(Color::White)),
            state: Arc::new(Mutex::new(State::new(5))),
            wait_move: Arc::new(Mutex::new(false)),
            wait_undo: Arc::new(Mutex::new(false)),
            request_undo: Arc::new(Mutex::new(false)),
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
                        Message::GameStart(color) => {
                            *data.color.lock().unwrap() = color;
                            game_start_sender.send(()).ok();
                            game_started = true;
                        },
                        Message::MoveRequest(ref new_state, _) => {
                            *data.state.lock().unwrap() = new_state.clone();
                            *data.wait_move.lock().unwrap() = true;

                            if !share_stdin {
                                *data.wait_undo.lock().unwrap() = false;

                                {
                                    let mut request_undo = data.request_undo.lock().unwrap();
                                    if *request_undo {
                                        *request_undo = false;
                                        println!("\n  Your opponent ignored your undo request.");
                                    }
                                }

                                prompt(&data);
                            }
                        },
                        Message::UndoRequest => {
                            {
                                let mut request_undo = data.request_undo.lock().unwrap();
                                if *request_undo {
                                    println!("\n  Your opponent accepted your undo request.");
                                    *request_undo = false;
                                } else {
                                    *data.wait_undo.lock().unwrap() = true;
                                    println!("\n  Your opponent requests an undo.");
                                }
                            }

                            prompt(&data);
                        },
                        Message::RemoveUndoRequest => if *data.wait_undo.lock().unwrap() {
                            println!("\n  Your opponent has removed their undo request.");
                            *data.wait_undo.lock().unwrap() = false;
                            prompt(&data);
                        },
                        _ => (),
                    }

                    if game_started && share_stdin && child.is_none() && waiting(&data) {
                        let sender = sender.clone();
                        let data = data.clone();

                        child = Some(thread::spawn(move || {
                            loop {
                                if waiting(&data) {
                                    prompt(&data);
                                    process_command(&sender, &data, share_stdin);
                                } else {
                                    break;
                                }
                            }
                        }));
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
                    process_command(&sender, &data, share_stdin);

                    if *data.wait_move.lock().unwrap() {
                        prompt(&data)
                    }
                }
            });
        }

        fn waiting(data: &Data) -> bool {
            *data.wait_move.lock().unwrap() ||
            *data.wait_undo.lock().unwrap()
        }

        fn process_command(sender: &Sender<Message>, data: &Data, share_stdin: bool) {
            let input = {
                let mut input = String::new();
                match io::stdin().read_line(&mut input) {
                    Ok(_) => input.trim().to_string(),
                    Err(error) => panic!("Error: {}", error),
                }
            };

            if input == "help" {
                println!("Commands:");
                println!("  undo        - Requests an undo from your opponent of the most recent move.");
                println!("  cancel undo - Removes your request for an undo.");
                println!("  quit        - Forfeits the game and exits.");
                println!("  [move]      - Enters your move in PTN format, e.g. a1, or 3d3<12.");
                return;
            }

            if input == "quit" {
                sender.send(Message::Quit(*data.color.lock().unwrap())).ok();
                return;
            }

            {
                let mut request_undo = data.request_undo.lock().unwrap();
                if *request_undo {
                    if input == "cancel undo" {
                        *request_undo = false;
                        sender.send(Message::RemoveUndoRequest).ok();
                    } else {
                        println!("  You've requested an undo from your opponent.");
                        println!("  Enter \"cancel undo\" if you'd like to withdraw your request.");
                    }
                    return;
                } else if input == "undo" {
                    if !share_stdin {
                        if !(*data.wait_undo.lock().unwrap()) {
                            *request_undo = true;
                            sender.send(Message::UndoRequest).ok();
                        } else {
                            println!("  Your opponent is already waiting for a response to an undo request.");
                            println!("  Enter \"accept\" to accept their request.");
                        }
                        return;
                    } else {
                        loop {
                            println!("  Your opponent requests an undo.");
                            println!("  Do you accept?");
                            print!("Enter \"accept\" or \"reject\": ");
                            io::stdout().flush().ok();

                            let input = {
                                let mut input = String::new();
                                match io::stdin().read_line(&mut input) {
                                    Ok(_) => input.trim().to_string(),
                                    Err(error) => panic!("Error: {}", error),
                                }
                            };

                            if input == "accept" {
                                *data.wait_move.lock().unwrap() = false;
                                sender.send(Message::Undo).ok();
                                break;
                            } else if input == "reject" {
                                break;
                            }
                        }
                        return;
                    }
                }
            }

            {
                let mut wait_undo = data.wait_undo.lock().unwrap();
                if *wait_undo {
                    if input == "accept" {
                        *wait_undo = false;
                        sender.send(Message::UndoRequest).ok();
                        sender.send(Message::Undo).ok();
                        return;
                    } else if input == "reject" {
                        *wait_undo = false;
                        return;
                    }
                }
            }

            {
                let mut wait_move = data.wait_move.lock().unwrap();
                if *wait_move {
                    if let Some(ply) = parse_ply(&input, &*data.state.lock().unwrap()) {
                        *wait_move = false;
                        *data.wait_undo.lock().unwrap() = false;
                        sender.send(Message::MoveResponse(ply)).ok();
                    }
                }
            }
        }

        fn prompt(data: &Data) {
            if *data.wait_undo.lock().unwrap() {
                print!("Enter command (Your opponent requested an undo): ");
            } else if *data.request_undo.lock().unwrap() {
                print!("Enter command (You requested an undo): ");
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
