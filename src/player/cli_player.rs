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

use std::any::Any;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use zero_sum::impls::tak::{Color, Piece, Ply, State};
use zero_sum::State as StateTrait;

use game::Message;
use player::Player;

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
    fn initialize(&mut self, to_game: Sender<(Color, Message)>, opponent: &Player) -> Result<Sender<Message>, String> {
        let share_stdin = opponent.as_any().is::<CliPlayer>();
        let (sender, receiver) = mpsc::channel();

        let data = Arc::new(Mutex::new(Data {
            color: Color::White,
            state: State::new(5),
            move_wait: false,
            undo_wait: false,
            undo_request: false,
        }));

        let (game_start, game_started) = mpsc::channel();
        let (input_sender, input_receiver) = mpsc::channel();

        // Message handler
        {
            let to_game = to_game.clone();
            let data = data.clone();
            let input_sender = input_sender.clone();

            thread::spawn(move || {
                for message in receiver.iter() {
                    match message {
                        Message::GameStart(assigned_color) => {
                            data.lock().unwrap().color = assigned_color;
                            game_start.send(()).ok();
                        },
                        Message::MoveResponse(_) => if !share_stdin {
                            prompt(&data);
                        },
                        Message::MoveRequest(state) => {
                            data.lock().unwrap().state = state;
                            data.lock().unwrap().move_wait = true;

                            if !share_stdin {
                                data.lock().unwrap().undo_wait = false;

                                {
                                    let mut data = data.lock().unwrap();
                                    if data.undo_request {
                                        data.undo_request = false;
                                        println!("\n  Your opponent ignored your undo request.");
                                    }
                                }
                            } else {
                                let data = data.clone();
                                let input_sender = input_sender.clone();

                                thread::spawn(move || {
                                    prompt(&data);
                                    fetch_input(&input_sender);
                                });
                            }
                        },
                        Message::UndoRequest => if share_stdin {
                            to_game.send((data.lock().unwrap().color, Message::UndoAccept)).ok();
                        } else {
                            println!("\n  Your opponent requests an undo.");
                            println!("  Enter \"accept\" or \"reject\".");
                            data.lock().unwrap().undo_wait = true;
                            prompt(&data);
                        },
                        Message::UndoAccept => if !share_stdin {
                            println!("\n  Your opponent has accepted your undo request.");
                            data.lock().unwrap().undo_request = false;
                            prompt(&data);
                        },
                        Message::UndoRemove => if !share_stdin {
                            println!("\n  Your opponent has removed their undo request.");
                            data.lock().unwrap().undo_wait = false;
                            prompt(&data);
                        },
                        Message::GameOver => {
                            input_sender.send(Input::Quit).ok();
                            break;
                        },
                        _ => (),
                    }
                }
            });
        }

        // Input handler
        thread::spawn(move || {
            game_started.recv().ok();

            // Input collector
            if !share_stdin {
                let data = data.clone();
                let input_sender = input_sender.clone();

                thread::spawn(move || {
                    loop {
                        thread::sleep(Duration::from_millis(100));
                        prompt(&data);
                        fetch_input(&input_sender);
                    }
                });
            }

            for input in input_receiver.iter() {
                match input {
                    Input::String(string) => parse_input(&string, &data, &input_sender, &to_game, share_stdin),
                    Input::Quit => break,
                }
            }
        });

        Ok(sender)
    }

    fn get_name(&self) -> String {
        self.name.clone()
    }

    fn as_any(&self) -> &Any {
        self
    }
}

struct Data {
    color: Color,
    state: State,
    move_wait: bool,
    undo_wait: bool,
    undo_request: bool,
}

fn prompt(data: &Arc<Mutex<Data>>) {
    if data.lock().unwrap().undo_wait {
        print!("Enter command (Your opponent requested an undo): ");
    } else if data.lock().unwrap().undo_request {
        print!("Enter command (You requested an undo): ");
    } else {
        print!("Enter command: ");
    }

    io::stdout().flush().ok();
}

enum Input {
    String(String),
    Quit,
}

fn fetch_input(input_sender: &Sender<Input>) {
    let input = {
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => input.trim().to_string(),
            Err(error) => panic!("Error: {}", error),
        }
    };

    input_sender.send(Input::String(input)).ok();
}

fn parse_input(string: &String, data: &Arc<Mutex<Data>>, input_sender: &Sender<Input>, to_game: &Sender<(Color, Message)>, share_stdin: bool) {
    if string == "help" {
        println!("Commands:");
        println!("  undo        - Sends a request to your opponent for an undo of the most recent move.");
        println!("  cancel undo - Removes your request for an undo.");
        println!("  quit        - Forfeits the game and exits.");
        println!("  [move]      - Enters your move in PTN format, e.g. a1, or 3d3<12.");

        if share_stdin {
            prompt(data);
            fetch_input(input_sender);
        }
    } else if string == "quit" {
        to_game.send((data.lock().unwrap().color, Message::GameOver)).ok();
    } else if string == "undo" {
        if share_stdin {
            to_game.send((data.lock().unwrap().color, Message::UndoRequest)).ok();
        } else {
            let mut data = data.lock().unwrap();

            if data.undo_request {
                println!("  You've already requested an undo from your opponent.");
                println!("  Enter \"cancel undo\" if you'd like to cancel your request.");
            } else if data.undo_wait {
                println!("  Your opponent is already waiting for a response to an undo request.");
                println!("  Enter \"accept\" to accept their request.");
            } else {
                data.undo_request = true;
                to_game.send((data.color, Message::UndoRequest)).ok();
            }
        }
    } else if string == "accept" {
        if share_stdin {
            prompt(data);
            fetch_input(input_sender);
            return;
        }

        let mut data = data.lock().unwrap();

        if data.undo_wait {
            data.undo_wait = false;
            to_game.send((data.color, Message::UndoAccept)).ok();
        }
    } else if string == "reject" {
        if share_stdin {
            prompt(data);
            fetch_input(input_sender);
            return;
        }

        let mut data = data.lock().unwrap();

        data.undo_wait = false;
    } else if string == "cancel undo" {
        if share_stdin {
            prompt(data);
            fetch_input(input_sender);
            return;
        }

        let mut data = data.lock().unwrap();

        if data.undo_request {
            data.undo_request = false;
            to_game.send((data.color, Message::UndoRemove)).ok();
        } else if data.undo_wait {
            println!("  You have not requested an undo.");
            println!("  If you would like to reject your opponent's undo request, enter \"reject\",");
            println!("  or make a move.");
        }
    } else {
        let mut bad_move = false;

        {
            let mut data = data.lock().unwrap();

            if data.move_wait {
                if let Some(ply) = parse_ply(&string, &data.state) {
                    if data.undo_request {
                        data.undo_request = false;
                        to_game.send((data.color, Message::UndoRemove)).ok();
                    }

                    data.move_wait = false;
                    to_game.send((data.color, Message::MoveResponse(ply))).ok();
                } else if share_stdin {
                    bad_move = true;
                }
            }
        }

        if bad_move == true {
            prompt(data);
            fetch_input(input_sender);
        }
    }
}

fn parse_ply(string: &str, state: &State) -> Option<Ply> {
    let mut state = state.clone();
    let board_size = state.board.len();

    let player_color = if state.ply_count % 2 == 0 {
        Color::White
    } else {
        Color::Black
    };

    let ply = match Ply::from_ptn(string, player_color) { // XXX Move this error checking into State?
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

    if let Err(error) = state.execute_ply(Some(&ply)) {
        println!("  {}", error);
        None
    } else {
        Some(ply)
    }
}
