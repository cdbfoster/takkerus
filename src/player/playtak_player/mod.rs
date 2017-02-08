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
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;

use zero_sum::impls::tak::{Color, Ply};

use game::Message;
use player::Player;

pub struct PlayTakPlayer {
    host: String,
    username: String,
    password: String,
    game_type: GameType,

    id: Arc<Mutex<String>>,
    name: Arc<Mutex<String>>,
    size: Arc<Mutex<usize>>,
    color: Arc<Mutex<Color>>,
    resume_plies: Arc<Mutex<Option<Vec<Ply>>>>,
}

impl PlayTakPlayer {
    pub fn new(host: &str, username: &str, password: &str, game_type: GameType) -> PlayTakPlayer {
        PlayTakPlayer {
            host: host.to_string(),
            username: username.to_string(),
            password: password.to_string(),
            game_type: game_type,
            id: Arc::new(Mutex::new(String::new())),
            name: Arc::new(Mutex::new(String::new())),
            size: Arc::new(Mutex::new(5)),
            color: Arc::new(Mutex::new(Color::White)),
            resume_plies: Arc::new(Mutex::new(None)),
        }
    }

    pub fn get_game_info(&self) -> (usize, Color, Option<Vec<Ply>>) {
        (*self.size.lock().unwrap(), *self.color.lock().unwrap(), self.resume_plies.lock().unwrap().clone())
    }
}

impl Player for PlayTakPlayer {
    fn initialize(&mut self, to_game: Sender<(Color, Message)>, _: &Player) -> Result<Sender<Message>, String> {
        let stream = match TcpStream::connect(self.host.as_str()) {
            Ok(stream) => Arc::new(Mutex::new(stream)),
            Err(_) => return Err(String::from("Could not connect to host.")),
        };

        println!("Connected.");

        let message_queue = MessageQueue::new();
        //let mut game_list = Vec::new();

        // Read from the stream into the message queue
        comm::start_reader(stream.lock().unwrap().try_clone().unwrap(), &message_queue);

        // Ping the server every 30 seconds
        comm::start_pinger(&stream);

        // Initialize client
        comm::write_client_name(&stream);
        try!(comm::login(&stream, &message_queue, self));
        let game_list = comm::initialize_game(&stream, &message_queue, self);

        // Start main handlers
        let (sender, state, undo_request, undo_wait) = comm::start_game_handler(&stream, self);
        comm::start_playtak_handler(&stream, message_queue, to_game, state, undo_request, undo_wait, self, game_list);

        Ok(sender)
    }

    fn get_name(&self) -> String {
        self.name.lock().unwrap().clone()
    }

    fn as_any(&self) -> &Any {
        self
    }
}

pub use self::game_type::GameType;
use self::message_queue::MessageQueue;

mod comm;
mod game_type;
mod message_queue;
mod parse;
