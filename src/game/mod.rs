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

use std::fmt;
use std::io::{self, Write};
use std::mem;
use std::sync::mpsc::{self, Sender};

use zero_sum::impls::tak::{Color, Ply, State};
use zero_sum::State as StateTrait;

use player::{self, Player};

pub struct Game {
    pub header: Header,
    pub plies: Vec<Ply>,

    p1: Option<Box<Player>>,
    p2: Option<Box<Player>>,

    p1_sender: Option<Sender<Message>>,
    p2_sender: Option<Sender<Message>>,
}

impl Game {
    pub fn new() -> Game {
        Game {
            header: Header::new(),
            plies: Vec::new(),
            p1: None,
            p2: None,
            p1_sender: None,
            p2_sender: None,
        }
    }

    pub fn into_state(header: Header, plies: Vec<Ply>) -> Result<State, String> {
        Game {
            header: header,
            plies: plies,
            p1: None, p2: None, p1_sender: None, p2_sender: None,
        }.to_state()
    }

    pub fn to_state(&self) -> Result<State, String> {
        let state = if self.header.tps.is_empty() {
            State::new(self.header.size)
        } else {
            if let Some(state) = State::from_tps(&format!("[TPS \"{}\"]", self.header.tps)) {
                state
            } else {
                return Err(String::from("Invalid TPS."))
            }
        };

        state.execute_plies(&self.plies)
    }

    pub fn add_player(&mut self, player: Box<Player>) -> Result<(), String> {
        if self.p1.is_none() {
            self.p1 = Some(player);
        } else if self.p2.is_none() {
            self.p2 = Some(player);
        } else {
            return Err(String::from("There are already two players in this game."));
        }

        Ok(())
    }

    pub fn play(&mut self) -> Result<(), String> {
        // Initialize players
        let (receiver, mut p1_playtak, mut p2_playtak) = {
            let p1 = if let Some(ref mut p1) = self.p1 {
                p1
            } else {
                return Err(String::from("There is no first player."))
            };

            let p2 = if let Some(ref mut p2) = self.p2 {
                p2
            } else {
                return Err(String::from("There is no second player."))
            };

            let p1_playtak = p1.as_any().is::<player::PlayTakPlayer>();
            let p2_playtak = p2.as_any().is::<player::PlayTakPlayer>();

            if p1_playtak && p2_playtak {
                return Err(String::from("There can only be one PlayTak player."));
            }

            let (sender, receiver) = mpsc::channel();

            match p1.initialize(sender.clone(), &**p2) {
                Ok(sender) => self.p1_sender = Some(sender),
                Err(error) => return Err(error),
            }

            match p2.initialize(sender, &**p1) {
                Ok(sender) => self.p2_sender = Some(sender),
                Err(error) => return Err(error),
            }

            (
                receiver,
                p1_playtak,
                p2_playtak,
            )
        };

        if p1_playtak || p2_playtak {
            let color = get_playtak_info(self).1;

            if (color == Color::White && p2_playtak) ||
               (color == Color::Black && p1_playtak) {
                mem::swap(&mut p1_playtak, &mut p2_playtak);
                mem::swap(&mut self.p1, &mut self.p2);
                mem::swap(&mut self.p1_sender, &mut self.p2_sender);
            }
        }

        if let Some(ref p1) = self.p1 {
            self.header.p1 = p1.get_name();
        }

        if let Some(ref p2) = self.p2 {
            self.header.p2 = p2.get_name();
        }

        if p1_playtak || p2_playtak {
            self.header.site = String::from("PlayTak.com");

            {
                let (size, _, plies) = get_playtak_info(self);

                self.header.size = size;

                if let Some(plies) = plies {
                    self.plies = plies;
                }
            }

            self.header.round = format!("{}", logger::get_round_number(self));
        } else if let Ok((header, plies)) = logger::read_tmp_file() {
            println!("There is a game in progress.\n");

            if !header.p1.is_empty() && !header.p2.is_empty() {
                print!("  {} vs {}", header.p1, header.p2);
            }

            if !header.date.is_empty() {
                print!("  {}", header.date);
            }

            let game = Game {
                header: header,
                plies: plies,
                p1: None, p2: None, p1_sender: None, p2_sender: None,
            };

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
                            println!("Here");
                            self.header = game.header;
                            self.plies = game.plies;
                            break;
                        } else if response == "n" {
                            self.header.round = format!("{}", logger::get_round_number(self));
                            break;
                        }
                    },
                    Err(e) => panic!("Error: {}", e),
                }
            }
        } else {
            self.header.round = format!("{}", logger::get_round_number(self));
        }

        // Start game
        self.send_message(Color::White, Message::GameStart(Color::White));
        self.send_message(Color::Black, Message::GameStart(Color::Black));

        print_game(self);

        {
            let state = self.to_state().unwrap();

            self.send_message(
                if state.ply_count % 2 == 0 {
                    Color::White
                } else {
                    Color::Black
                },
                Message::MoveRequest(state),
            );
        }

        let mut undo_requested = None;

        // Run game
        for (color, message) in receiver.iter() {
            match message {
                Message::MoveResponse(ply) => {
                    if color != if self.plies.len() % 2 == 0 {
                        Color::White
                    } else {
                        Color::Black
                    } {
                        continue;
                    }

                    if let Some(undo_color) = undo_requested.clone() {
                        if color == undo_color {
                            undo_requested = None;
                        }
                    }

                    let state = self.to_state().unwrap();

                    match state.execute_ply(&ply) {
                        Ok(next) => {
                            self.plies.push(ply.clone());
                            print_game(self);

                            logger::write_tmp_file(self);

                            self.send_message(color.flip(), Message::MoveResponse(ply));

                            if next.check_resolution().is_some() {
                                self.send_message(color.flip(), Message::GameOver);
                                self.send_message(color, Message::GameOver);

                                logger::finalize_tmp_file();
                            } else {
                                self.send_message(color.flip(), Message::MoveRequest(next));
                            }
                        },
                        _ => {
                            println!("Bad move");
                            self.send_message(color, Message::MoveRequest(state));
                        },
                    }
                },
                Message::UndoRequest => if undo_requested.is_none() {
                    undo_requested = Some(color);
                    self.send_message(color.flip(), Message::UndoRequest);
                },
                Message::UndoAccept => if let Some(undo_color) = undo_requested.clone() {
                    if color == undo_color.flip() {
                        undo_requested = None;

                        self.plies.pop();
                        print_game(self);

                        logger::write_tmp_file(self);

                        self.send_message(undo_color, Message::UndoAccept);
                        self.send_message(
                            if self.plies.len() % 2 == 0 {
                                Color::White
                            } else {
                                Color::Black
                            },
                            Message::MoveRequest(self.to_state().unwrap()),
                        );
                    }
                },
                Message::UndoRemove => if let Some(undo_color) = undo_requested.clone() {
                    if color == undo_color {
                        undo_requested = None;
                        self.send_message(color.flip(), Message::UndoRemove);
                    }
                },
                Message::GameOver => {
                    self.send_message(color.flip(), Message::GameOver);
                    self.send_message(color, Message::GameOver);
                },
                Message::Special(string) => if string == "Disconnected" || string == "0-1" || string == "1-0" {
                    if string == "0-1" || string == "1-0" {
                        self.header.result = string.clone();
                        logger::write_tmp_file(self);
                        logger::finalize_tmp_file();
                    }

                    // If this is an early disconnect/end, end the game.  Otherwise, it's already over.
                    if self.to_state().unwrap().check_resolution().is_none() {
                        self.send_message(color.flip(), Message::GameOver);
                        self.send_message(color, Message::GameOver);
                    }
                },
                _ => (),
            }
        }

        Ok(())
    }

    fn send_message(&self, color: Color, message: Message) {
        match color {
            Color::White => if let Some(ref sender) = self.p1_sender {
                sender.send(message).ok();
            } else {
                panic!("No player 1 sender!");
            },
            Color::Black => if let Some(ref sender) = self.p2_sender {
                sender.send(message).ok();
            } else {
                panic!("No player 2 sender!");
            },
        }
    }
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}\r\n", self.header).ok();

        for turn in 0..(self.plies.len() + 1) / 2 {
            write!(f, "{:2}. {:7} ", turn + 1, self.plies[turn * 2].to_ptn()).ok();

            if turn * 2 + 1 < self.plies.len() {
                write!(f, "{}", self.plies[turn * 2 + 1].to_ptn()).ok();
            }

            write!(f, "\r\n").ok();
        }

        write!(f, "\r\n")
    }
}

fn print_game(game: &Game) {
    let state = game.to_state().unwrap();

    println!("\n--------------------------------------------------");
    println!("{}", state);

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

    if state.check_resolution().is_some() {
        println!("Final state:     {}\n", ptn);
    } else if state.ply_count > 0 {
        println!("Previous {}:   {}\n", if state.ply_count % 2 == 0 {
            "turn"
        } else {
            "move"
        }, ptn);
    } else {
        println!("\n");
    }
}

fn get_playtak_info(game: &Game) -> (usize, Color, Option<Vec<Ply>>) {
    if let Some(ref p1) = game.p1 {
        match p1.as_any().downcast_ref::<player::PlayTakPlayer>() {
            Some(player) => return player.get_game_info(),
            None => (),
        }
    }

    if let Some(ref p2) = game.p2 {
        match p2.as_any().downcast_ref::<player::PlayTakPlayer>() {
            Some(player) => return player.get_game_info(),
            None => (),
        }
    }

    (5, Color::White, None)
}

pub use self::header::Header;
pub use self::message::Message;

mod header;
pub mod logger;
mod message;
